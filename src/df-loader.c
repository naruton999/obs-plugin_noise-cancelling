#include "df-loader.h"

#include <obs-module.h>
#include <plugin-support.h>
#include <util/platform.h>

#include <stdio.h>
#include <string.h>

#ifdef _WIN32
#define DF_FFI_LIBRARY "df_ffi.dll"
#define DF_PATH_SEPARATOR '\\'
#elif defined(__APPLE__)
#define DF_FFI_LIBRARY "libdf_ffi.dylib"
#define DF_PATH_SEPARATOR '/'
#else
#define DF_FFI_LIBRARY "libdf_ffi.so"
#define DF_PATH_SEPARATOR '/'
#endif

#define DF_PATH_MAX 4096

static bool load_symbol(void *module, const char *name, void *target, size_t target_size)
{
	void *symbol = os_dlsym(module, name);

	if (!symbol) {
		obs_log(LOG_ERROR, "df_ffi: missing symbol %s", name);
		return false;
	}

	if (target_size < sizeof(symbol)) {
		obs_log(LOG_ERROR, "df_ffi: storage for %s is too small", name);
		return false;
	}

	memcpy(target, &symbol, sizeof(symbol));
	return true;
}

bool df_api_load(struct df_api *api)
{
	char binary_path[DF_PATH_MAX];
	char library_path[DF_PATH_MAX];
	const char *module_path;
	char *separator;
	size_t path_length;

	memset(api, 0, sizeof(*api));

	module_path = obs_get_module_binary_path(obs_current_module());
	if (!module_path) {
		obs_log(LOG_ERROR, "df_ffi: could not resolve the plugin binary path");
		return false;
	}

	path_length = strlen(module_path);
	if (path_length >= sizeof(binary_path)) {
		obs_log(LOG_ERROR, "df_ffi: plugin binary path is too long");
		return false;
	}

	memcpy(binary_path, module_path, path_length + 1);

	separator = strrchr(binary_path, '/');
#ifdef _WIN32
	{
		char *backslash = strrchr(binary_path, '\\');
		if (!separator || (backslash && backslash > separator))
			separator = backslash;
	}
#endif

	if (!separator) {
		obs_log(LOG_ERROR, "df_ffi: could not determine the plugin directory");
		return false;
	}

	*separator = '\0';

	if (snprintf(library_path, sizeof(library_path), "%s%c%s", binary_path, DF_PATH_SEPARATOR, DF_FFI_LIBRARY) >=
	    (int)sizeof(library_path)) {
		obs_log(LOG_ERROR, "df_ffi: library path is too long");
		return false;
	}

	api->module = os_dlopen(library_path);
	if (!api->module) {
		obs_log(LOG_ERROR, "df_ffi: failed to load %s", library_path);
		return false;
	}

	if (!load_symbol(api->module, "dfx_create", &api->create, sizeof(api->create)) ||
	    !load_symbol(api->module, "dfx_destroy", &api->destroy, sizeof(api->destroy)) ||
	    !load_symbol(api->module, "dfx_hop_size", &api->hop_size, sizeof(api->hop_size)) ||
	    !load_symbol(api->module, "dfx_sample_rate", &api->sample_rate, sizeof(api->sample_rate)) ||
	    !load_symbol(api->module, "dfx_process", &api->process, sizeof(api->process)) ||
	    !load_symbol(api->module, "dfx_set_atten_lim", &api->set_atten_lim, sizeof(api->set_atten_lim)) ||
	    !load_symbol(api->module, "dfx_set_post_filter_beta", &api->set_post_filter_beta,
			 sizeof(api->set_post_filter_beta))) {
		os_dlclose(api->module);
		memset(api, 0, sizeof(*api));
		return false;
	}

	obs_log(LOG_INFO, "df_ffi loaded from %s", library_path);
	return true;
}

void df_api_unload(struct df_api *api)
{
	if (!api)
		return;

	if (api->module)
		os_dlclose(api->module);

	memset(api, 0, sizeof(*api));
}
