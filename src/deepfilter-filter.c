#include "deepfilter-filter.h"

#include "df-loader.h"

#include <obs-module.h>
#include <plugin-support.h>
#include <util/threading.h>

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define DF_CHANNELS 2
#define DF_OUTPUT_FIFO_EXTRA 32768
#define DF_DEFAULT_ATTEN_LIM_DB 100.0f
#define DF_POST_FILTER_BETA 0.02f
#define DF_SAMPLE_RATE 48000

struct df_channel {
	void *handle;
	float *input;
	size_t input_len;
	size_t hop;
	float *processed;
	float *output;
	size_t output_capacity;
	size_t output_head;
	size_t output_count;
};

struct deepfilter_ctx {
	struct df_api api;
	bool api_loaded;
	bool available;
	bool failed;

	struct df_channel channels[DF_CHANNELS];

	volatile long atten_lim_x100;
	volatile long post_filter_beta_x1000;
	long applied_atten_lim_x100;
	long applied_post_filter_beta_x1000;
};

static bool df_channel_init(struct df_channel *channel, const struct df_api *api, float atten_lim_db,
			    float post_filter_beta)
{
	size_t sample_rate;

	memset(channel, 0, sizeof(*channel));

	channel->handle = api->create(atten_lim_db, post_filter_beta);
	if (!channel->handle) {
		obs_log(LOG_ERROR, "DeepFilterNet: failed to create a DF state");
		return false;
	}

	channel->hop = api->hop_size(channel->handle);
	sample_rate = api->sample_rate(channel->handle);
	if (channel->hop == 0 || sample_rate != DF_SAMPLE_RATE) {
		obs_log(LOG_ERROR, "DeepFilterNet: unsupported model parameters (hop=%zu, sr=%zu)", channel->hop,
			sample_rate);
		api->destroy(channel->handle);
		memset(channel, 0, sizeof(*channel));
		return false;
	}

	channel->input = bzalloc(channel->hop * sizeof(float));
	channel->processed = bzalloc(channel->hop * sizeof(float));
	channel->output_capacity = channel->hop * 2 + DF_OUTPUT_FIFO_EXTRA;
	channel->output = bzalloc(channel->output_capacity * sizeof(float));

	if (!channel->input || !channel->processed || !channel->output) {
		obs_log(LOG_ERROR, "DeepFilterNet: failed to allocate audio buffers");
		bfree(channel->input);
		bfree(channel->processed);
		bfree(channel->output);
		api->destroy(channel->handle);
		memset(channel, 0, sizeof(*channel));
		return false;
	}

	/* Prime the output FIFO with one hop of silence. */
	channel->output_count = channel->hop;
	return true;
}

static void df_channel_destroy(struct df_channel *channel, const struct df_api *api)
{
	if (!channel)
		return;

	if (channel->handle && api->destroy)
		api->destroy(channel->handle);

	bfree(channel->input);
	bfree(channel->processed);
	bfree(channel->output);
	memset(channel, 0, sizeof(*channel));
}

static void df_channel_reset(struct df_channel *channel)
{
	if (!channel || !channel->handle)
		return;

	channel->input_len = 0;
	channel->output_head = 0;
	channel->output_count = channel->hop;
	memset(channel->output, 0, channel->output_capacity * sizeof(float));
}

static bool df_channel_push(struct df_channel *channel, const float *samples)
{
	size_t index;

	if (channel->output_count + channel->hop > channel->output_capacity)
		return false;

	index = (channel->output_head + channel->output_count) % channel->output_capacity;
	for (size_t i = 0; i < channel->hop; i++) {
		channel->output[index] = samples[i];
		index++;
		if (index == channel->output_capacity)
			index = 0;
	}

	channel->output_count += channel->hop;
	return true;
}

static float df_channel_pop(struct df_channel *channel)
{
	float sample;

	if (channel->output_count == 0)
		return 0.0f;

	sample = channel->output[channel->output_head];
	channel->output_head++;
	if (channel->output_head == channel->output_capacity)
		channel->output_head = 0;
	channel->output_count--;

	return sample;
}

static bool df_channel_process(struct df_channel *channel, const struct df_api *api, float *samples, uint32_t frames)
{
	for (uint32_t i = 0; i < frames; i++) {
		channel->input[channel->input_len++] = samples[i];

		if (channel->input_len == channel->hop) {
			if (api->process(channel->handle, channel->input, channel->processed) != 0)
				return false;
			if (!df_channel_push(channel, channel->processed))
				return false;
			channel->input_len = 0;
		}
	}

	for (uint32_t i = 0; i < frames; i++)
		samples[i] = df_channel_pop(channel);

	return true;
}

static const char *deepfilter_get_name(void *unused)
{
	(void)unused;
	return obs_module_text("DeepFilterName");
}

static void *deepfilter_create(obs_data_t *settings, obs_source_t *source)
{
	struct deepfilter_ctx *ctx;
	float atten_lim_db;
	bool post_filter;
	float post_filter_beta;

	(void)source;

	ctx = bzalloc(sizeof(*ctx));
	os_atomic_store_long(&ctx->atten_lim_x100, (long)(DF_DEFAULT_ATTEN_LIM_DB * 100.0f));
	os_atomic_store_long(&ctx->post_filter_beta_x1000, 0);
	ctx->applied_atten_lim_x100 = -1;
	ctx->applied_post_filter_beta_x1000 = -1;

	ctx->api_loaded = df_api_load(&ctx->api);
	if (!ctx->api_loaded) {
		obs_log(LOG_ERROR, "DeepFilterNet: df_ffi is unavailable; the filter will pass audio through");
		return ctx;
	}

	atten_lim_db = settings ? (float)obs_data_get_double(settings, "atten_lim_db") : DF_DEFAULT_ATTEN_LIM_DB;
	post_filter = settings ? obs_data_get_bool(settings, "post_filter") : false;
	post_filter_beta = post_filter ? DF_POST_FILTER_BETA : 0.0f;

	if (atten_lim_db < 0.0f)
		atten_lim_db = 0.0f;
	if (atten_lim_db > 100.0f)
		atten_lim_db = 100.0f;

	os_atomic_store_long(&ctx->atten_lim_x100, (long)(atten_lim_db * 100.0f));
	os_atomic_store_long(&ctx->post_filter_beta_x1000, (long)(post_filter_beta * 1000.0f));

	ctx->available = df_channel_init(&ctx->channels[0], &ctx->api, atten_lim_db, post_filter_beta);
	if (!ctx->available) {
		obs_log(LOG_ERROR, "DeepFilterNet: failed to initialize the mono DF state");
		return ctx;
	}

	if (!df_channel_init(&ctx->channels[1], &ctx->api, atten_lim_db, post_filter_beta))
		obs_log(LOG_WARNING,
			"DeepFilterNet: failed to initialize the stereo DF state; right channel will pass through");

	obs_log(LOG_INFO, "DeepFilterNet: initialized (hop=%zu, sr=%d)", ctx->channels[0].hop, DF_SAMPLE_RATE);

	return ctx;
}

static void deepfilter_destroy(void *data)
{
	struct deepfilter_ctx *ctx = data;

	if (!ctx)
		return;

	for (size_t i = 0; i < DF_CHANNELS; i++)
		df_channel_destroy(&ctx->channels[i], &ctx->api);

	df_api_unload(&ctx->api);
	bfree(ctx);
}

static void deepfilter_update(void *data, obs_data_t *settings)
{
	struct deepfilter_ctx *ctx = data;
	float atten_lim_db;
	bool post_filter;
	float post_filter_beta;

	if (!ctx || !settings)
		return;

	atten_lim_db = (float)obs_data_get_double(settings, "atten_lim_db");
	post_filter = obs_data_get_bool(settings, "post_filter");
	post_filter_beta = post_filter ? DF_POST_FILTER_BETA : 0.0f;

	if (atten_lim_db < 0.0f)
		atten_lim_db = 0.0f;
	if (atten_lim_db > 100.0f)
		atten_lim_db = 100.0f;

	os_atomic_store_long(&ctx->atten_lim_x100, (long)(atten_lim_db * 100.0f));
	os_atomic_store_long(&ctx->post_filter_beta_x1000, (long)(post_filter_beta * 1000.0f));
}

static void deepfilter_activate(void *data)
{
	struct deepfilter_ctx *ctx = data;

	if (!ctx)
		return;

	for (size_t i = 0; i < DF_CHANNELS; i++)
		df_channel_reset(&ctx->channels[i]);

	ctx->failed = false;
	ctx->applied_atten_lim_x100 = -1;
	ctx->applied_post_filter_beta_x1000 = -1;
}

static void deepfilter_deactivate(void *data)
{
	(void)data;
}

static void deepfilter_get_defaults(obs_data_t *settings)
{
	obs_data_set_default_double(settings, "atten_lim_db", DF_DEFAULT_ATTEN_LIM_DB);
	obs_data_set_default_bool(settings, "post_filter", false);
}

static obs_properties_t *deepfilter_get_properties(void *data)
{
	struct deepfilter_ctx *ctx = data;
	obs_properties_t *props = obs_properties_create();

	obs_properties_add_float_slider(props, "atten_lim_db", obs_module_text("Attenuation"), 0.0, 100.0, 1.0);
	obs_properties_add_bool(props, "post_filter", obs_module_text("PostFilter"));

	if (ctx && !ctx->available)
		obs_properties_add_text(props, "status", obs_module_text("Unavailable"), OBS_TEXT_INFO);

	return props;
}

static struct obs_audio_data *deepfilter_filter_audio(void *data, struct obs_audio_data *audio)
{
	struct deepfilter_ctx *ctx = data;
	long atten_lim_x100;
	long post_filter_beta_x1000;
	float *left;
	uint32_t frames;

	if (!ctx || !ctx->available || ctx->failed || !audio || audio->frames == 0 || !audio->data[0])
		return audio;

	frames = audio->frames;
	atten_lim_x100 = os_atomic_load_long(&ctx->atten_lim_x100);
	post_filter_beta_x1000 = os_atomic_load_long(&ctx->post_filter_beta_x1000);

	if (atten_lim_x100 != ctx->applied_atten_lim_x100) {
		float atten_lim_db = (float)atten_lim_x100 / 100.0f;

		for (size_t i = 0; i < DF_CHANNELS; i++) {
			if (ctx->channels[i].handle && ctx->api.set_atten_lim)
				ctx->api.set_atten_lim(ctx->channels[i].handle, atten_lim_db);
		}

		ctx->applied_atten_lim_x100 = atten_lim_x100;
	}

	if (post_filter_beta_x1000 != ctx->applied_post_filter_beta_x1000) {
		float beta = (float)post_filter_beta_x1000 / 1000.0f;

		for (size_t i = 0; i < DF_CHANNELS; i++) {
			if (ctx->channels[i].handle && ctx->api.set_post_filter_beta)
				ctx->api.set_post_filter_beta(ctx->channels[i].handle, beta);
		}

		ctx->applied_post_filter_beta_x1000 = post_filter_beta_x1000;
	}

	left = (float *)audio->data[0];
	if (!df_channel_process(&ctx->channels[0], &ctx->api, left, frames)) {
		ctx->failed = true;
		obs_log(LOG_ERROR, "DeepFilterNet: processing failed; bypassing audio");
		return audio;
	}

	if (audio->data[1] && ctx->channels[1].handle) {
		float *right = (float *)audio->data[1];

		if (!df_channel_process(&ctx->channels[1], &ctx->api, right, frames)) {
			ctx->failed = true;
			obs_log(LOG_ERROR, "DeepFilterNet: processing failed; bypassing audio");
			return audio;
		}
	}

	return audio;
}

struct obs_source_info deepfilter_filter = {
	.id = "deepfilter_noise_cancelling",
	.type = OBS_SOURCE_TYPE_FILTER,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = deepfilter_get_name,
	.create = deepfilter_create,
	.destroy = deepfilter_destroy,
	.update = deepfilter_update,
	.get_defaults = deepfilter_get_defaults,
	.get_properties = deepfilter_get_properties,
	.activate = deepfilter_activate,
	.deactivate = deepfilter_deactivate,
	.filter_audio = deepfilter_filter_audio,
};
