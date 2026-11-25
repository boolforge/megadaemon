# Matriz de portabilidad: drivers proprietary (5.4 Samsung) -> 5.10/5.15/6.1/6.6/7.3

Generado con `tools/driver_port_matrix.py` sobre el C decompilado de `/workspace/output/mass_decomp`
(el C decompilado NO se versiona).

## Por que no vale con solo recompilar

| API | Cambio relevante |
|---|---|
| `ioctl_magic` | _IO/_IOR/_IOW/_IORW: 5.4->7.3 sin cambios; el ioctl de V4L2/VS4L se mantiene |
| `vfs_read_write` | read/write -> read_iter/write_iter: 5.9+: file_operations con read_iter/write_iter; cambia la firma |
| `poll_select` | poll/select -> poll_request: 5.9+: ->poll() con poll_request; 6.11: ->poll() cambia otra vez |
| `mmap` | mmap con vm_ops: 6.x: vm_flags, y read_folio/write_folio en 6.5+ |
| `drm` | struct drm_driver / legacy ioctl: 6.x: drm_device, drm_mode_config_funcs; los ioctl legacy migran a DRM_IOCTL_* |
| `v4l2` | struct video_device: 6.x: media controller 2.0 (v4l2_subdev, media_pad) |
| `input` | struct input_dev: 6.2+: input_device_abs_set_up deprecado; 6.12+: nuevo input API |
| `netlink` | netlink_kernel_create: 6.3+: netlink_policies obligatorias; 6.7+: per-netns |
| `sysfs_proc` | proc_create / sysfs attribute_group: 6.x: sin cambios de API, pero class->device_model migracion |
| `regmap` | regmap_init / regmap_config: 6.x: sin cambios |
| `clk_gpio` | clk_prepare_enable / gpio_request: 6.x: sin cambios |
| `i2c_spi` | i2c_driver / spi_driver: 6.9+: i2c_driver const; 6.12: cambios en adapters |
| `power` | regulator / dev_pm: 6.11+: dev_pm_ops refactor |
| `container_of` | container_of: sin cambios |
| `kABI` | struct layout de kernel structs: el kABI de GKI cambia entre 5.10/5.15/6.1/6.6: un modulo .ko de 5.4 NO carga en 5.15 |

## Clasificacion de los drivers por superficie de API

| driver | bytes | ioctls | APIs de kernel usadas | riesgo |
|---|---|---|---|---|
| `libbauthserver.so` | 1715602 | 105 | — | bajo |
| `libbauthserver.so__v64` | 1715458 | 105 | — | bajo |
| `libbauthserver__v64` | 1715602 | 105 | — | bajo |
| `libbauthserver.so__v32` | 1667344 | 82 | — | bajo |
| `libbauthserver__v32` | 1667344 | 82 | — | bajo |
| `libexynosgscaler.so` | 128488 | 58 | — | bajo |
| `libexynosgscaler.so__v32` | 115266 | 58 | — | bajo |
| `libexynosgscaler.so__v64` | 128488 | 58 | — | bajo |
| `libexynosgscaler__v32` | 115266 | 58 | — | bajo |
| `libexynosgscaler__v64` | 128488 | 58 | — | bajo |
| `libexynosdisplay.so__v64` | 2848125 | 38 | — | bajo |
| `libexynosdisplay__v64` | 2848125 | 38 | — | bajo |
| `libexynosv4l2.so` | 47385 | 32 | — | bajo |
| `libexynosv4l2.so__v32` | 44882 | 32 | — | bajo |
| `libexynosv4l2.so__v64` | 47385 | 32 | — | bajo |
| `libexynosv4l2__v32` | 44882 | 32 | — | bajo |
| `libexynosv4l2__v64` | 47385 | 32 | — | bajo |
| `libnetutils__v32s` | 99238 | 32 | — | bajo |
| `libsdp_sdk__v32s` | 28257 | 32 | — | bajo |
| `libsdp_sdk__v64s` | 26854 | 32 | — | bajo |
| `libexynosdisplay.so__v32` | 2166906 | 30 | — | bajo |
| `libexynosdisplay__v32` | 2166906 | 30 | — | bajo |
| `libion_exynos__v32` | 18836 | 28 | — | bajo |
| `libion_exynos__v64` | 19551 | 28 | — | bajo |
| `libsbwcwrapper.so` | 81741 | 25 | — | bajo |
| `libsbwcwrapper.so__v64` | 81741 | 25 | — | bajo |
| `libsbwcwrapper__v64` | 81741 | 25 | — | bajo |
| `libgpudataproducer.so__v32` | 5495918 | 22 | — | bajo |
| `libgpudataproducer.so__v64` | 3788998 | 22 | — | bajo |
| `libeden_osal__v32` | 70304 | 19 | — | bajo |
| `libeden_osal__v64` | 41808 | 19 | — | bajo |
| `libion__v32s` | 14826 | 18 | — | bajo |
| `libion__v64s` | 15234 | 18 | — | bajo |
| `libbpf__v32s` | 104923 | 17 | — | bajo |
| `libtinycompress__v32` | 33060 | 17 | — | bajo |
| `libtinycompress__v64` | 40501 | 17 | — | bajo |
| `libteecl.so__v32` | 168664 | 14 | — | bajo |
| `libteecl.so__v64` | 138043 | 14 | — | bajo |
| `libteecl__v32` | 168664 | 14 | — | bajo |
| `libteecl__v64` | 138043 | 14 | — | bajo |
| `libusbhost__v64s` | 47100 | 13 | — | bajo |
| `sensors.sensorhub.so__v64` | 735601 | 12 | — | bajo |
| `sensors.sensorhub__v32` | 683390 | 12 | — | bajo |
| `sensors.sensorhub__v64` | 735601 | 12 | — | bajo |
| `libusbhost__v32s` | 37909 | 11 | — | bajo |
| `libegis_fp_normal_sensor_test__v64` | 28939 | 10 | — | bajo |
| `libsdp_kekm__v32s` | 87360 | 10 | — | bajo |
| `libsync__v32s` | 11091 | 10 | — | bajo |
| `libsync__v64s` | 11106 | 10 | — | bajo |
| `libtui_service_jni__v64s` | 29375 | 9 | — | bajo |
| `libenn_dsp_dal.so` | 727509 | 8 | — | bajo |
| `libenn_dsp_dal__v32` | 630802 | 8 | — | bajo |
| `libenn_dsp_dal__v64` | 727509 | 8 | — | bajo |
| `libhmc1Lib.so__v32` | 172446 | 8 | — | bajo |
| `libhmc1Lib.so__v64` | 189214 | 8 | — | bajo |
| `libhmc1Lib__v32` | 172446 | 8 | — | bajo |
| `libhmc1Lib__v64` | 189214 | 8 | — | bajo |
| `libsdp_crypto__v32s` | 58402 | 8 | — | bajo |
| `libuinputcommand_jni__v64s` | 44918 | 8 | — | bajo |
| `libwifi-system-iface__v64s` | 40911 | 8 | — | bajo |
| `libchrome.so__sys32` | 5283568 | 7 | — | bajo |
| `libchrome.so__sys64` | 5418288 | 7 | — | bajo |
| `libchrome__v32s` | 5283568 | 7 | — | bajo |
| `libchrome__v64s` | 5418288 | 7 | — | bajo |
| `gralloc.default__v32` | 35200 | 6 | — | bajo |
| `gralloc.default__v64` | 31005 | 6 | — | bajo |
| `libaccauthentication_jni__v32s` | 22318 | 4 | — | bajo |
| `libaccauthentication_jni__v64s` | 27139 | 4 | — | bajo |
| `libartpalette-system__v32s` | 94384 | 4 | — | bajo |
| `libcfms.ssrm.samsung__v32s` | 17207 | 4 | — | bajo |
| `libcfms.ssrm.samsung__v64s` | 18407 | 4 | — | bajo |
| `libchrome.so__v64` | 5032203 | 4 | — | bajo |
| `libchrome__v64` | 5032203 | 4 | — | bajo |
| `libext4_utils__v64s` | 19323 | 4 | — | bajo |
| `ese_spi_nxp.so` | 275276 | 3 | — | bajo |
| `libchrome.so__v32` | 4785025 | 3 | — | bajo |
| `libchrome__v32` | 4785025 | 3 | — | bajo |
| `libdiskconfig__v64s` | 39639 | 3 | — | bajo |
| `libese-grdg.so` | 118098 | 3 | — | bajo |
| `libese-grdg.so__v64` | 118098 | 3 | — | bajo |
| `libese-grdg__v64` | 118098 | 3 | — | bajo |
| `libexynos_migov__v64` | 4855608 | 3 | — | bajo |
| `libalarm_jni__v32s` | 46961 | 2 | — | bajo |
| `libalarm_jni__v64s` | 37272 | 2 | — | bajo |
| `libext2_uuid__v64s` | 23633 | 2 | — | bajo |
| `libhwjsqz__v32` | 82178 | 2 | — | bajo |
| `libhwjsqz__v64` | 119219 | 2 | — | bajo |
| `libremotedesktopservice__v32s` | 75334 | 2 | — | bajo |
| `libshctrl__v64` | 7041 | 2 | — | bajo |
| `libstork_shared__v32` | 11793 | 2 | — | bajo |
| `libstork_shared__v64` | 14507 | 2 | — | bajo |
| `libtinyalsa__v32s` | 116018 | 2 | — | bajo |
| `pppol2tp-android__v64s` | 2352 | 2 | — | bajo |
| `sound_trigger.primary.exynos2100.so__v32` | 75445 | 2 | — | bajo |
| `sound_trigger.primary.exynos2100.so__v64` | 110619 | 2 | — | bajo |
| `sound_trigger.primary.exynos2100__v32` | 75445 | 2 | — | bajo |
| `sound_trigger.primary.exynos2100__v64` | 110619 | 2 | — | bajo |
| `libaudioproxy2.so__v32` | 445902 | 1 | — | bajo |
| `libaudioproxy2.so__v64` | 715933 | 1 | — | bajo |
| `libaudioproxy2__v32` | 445902 | 1 | — | bajo |
| `libaudioproxy2__v64` | 715933 | 1 | — | bajo |
| `libegis_fp_normal_sensor_test__v32` | 83844 | 1 | — | bajo |
| `libext2_misc__v64s` | 32678 | 1 | — | bajo |
| `libfsverity__v64s` | 41957 | 1 | — | bajo |
| `libgiantmscl__v32` | 66697 | 1 | — | bajo |
| `libgiantmscl__v64` | 57006 | 1 | — | bajo |
| `libhypermotion_core.so__v32` | 986446 | 1 | — | bajo |
| `libhypermotion_core.so__v64` | 1208049 | 1 | — | bajo |
| `libhypermotion_core__v32` | 986446 | 1 | — | bajo |
| `libhypermotion_core__v64` | 1208049 | 1 | — | bajo |
| `libvdis_core.so__v32` | 1252479 | 1 | — | bajo |
| `libvdis_core.so__v64` | 1553210 | 1 | — | bajo |
| `libvdis_core__v32` | 1252479 | 1 | — | bajo |
| `libvdis_core__v64` | 1553210 | 1 | — | bajo |
| `NPU.bin` | 1517513 | 0 | — | bajo |
| `OD_V2.1.6_01_26_QVGA.nnc` | 1925 | 0 | — | bajo |
| `OD_V2.3.7_02_05_VGA.nnc` | 1925 | 0 | — | bajo |
| `a2dpcom.bin` | 1837523 | 0 | — | bajo |
| `android.frameworks.cameraservice.common@2.0.so` | 0 | 0 | — | bajo |
| `android.frameworks.cameraservice.device@2.0.so` | 0 | 0 | — | bajo |

## Lectura

- Los drivers con `vfs` + `poll` + `mmap` (display, V4L2, NPU) **no** se pueden portar por recopilacion: hay que reescribirlos con la API moderna (read_iter/write_iter, poll_request, read_folio/write_folio).
- Un `.ko` de 5.4 **no carga** en 5.15/6.x por el kABI de GKI: el camino es **reimplementacion en codigo abierto** (que es lo que hace este repo), no parcheo binario.
- Los drivers con pocas APIs (regmap/clk/gpio/i2c) son los candidatos rapidos para el port.

