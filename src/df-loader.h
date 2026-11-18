#pragma once

#include <stdbool.h>
#include <stddef.h>

struct df_api {
	void *module;

	void *(*create)(float atten_lim_db, float post_filter_beta);
	void (*destroy)(void *handle);
	size_t (*hop_size)(const void *handle);
	size_t (*sample_rate)(const void *handle);
	int (*process)(void *handle, const float *input, float *output);
	void (*set_atten_lim)(void *handle, float db);
	void (*set_post_filter_beta)(void *handle, float beta);
};

bool df_api_load(struct df_api *api);
void df_api_unload(struct df_api *api);
