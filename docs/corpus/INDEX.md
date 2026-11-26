# Corpus de conversaciones — todos los agentes

- Sesiones indexadas: **91** de 97 registradas
- Mensajes volcados: **7358**
- Artefactos leídos: 96
- Artefactos sin asignación: 4
- Sesiones con paginación pendiente: **65**
- Sesiones sin cobertura alguna: **1**

## Ficheros

| fichero | contenido |
|---|---|
| `sessions/*.md` | transcripción completa por sesión (YAML + mensajes) |
| `MESSAGES.jsonl` | todos los mensajes aplanados, una línea JSON por mensaje |
| `KEYWORDS.tsv` | índice invertido término → nº documentos / nº sesiones |
| `SESSIONS_INDEX.tsv` | metadatos de las 97 sesiones (agente, título, fechas) |
| `COVERAGE.tsv` | cobertura real por sesión (páginas, msgs, completa, cursor) |
| `PAGINATE_PLAN.tsv` | llamadas `session_messages` pendientes con su cursor |
| `FACTS.md` | notas destiladas por tema con referencia a sesión/mensaje |

## Sesiones

| # | agente | fecha | msj | título | fichero |
|---|---|---|---|---|---|
| 1 | Mavis | 2026-08-05 | 2 | [ROL] Actúa como un Ingeniero | [`Mavis__ROL_Actúa_como_un_Ingeniero__42765510.md`](sessions/Mavis__ROL_Actúa_como_un_Ingeniero__42765510.md) |
| 2 | Mavis | 2026-08-16 | 2 | /deep-research  Please improve | [`Mavis__deep-research_Please_improve__43147106.md`](sessions/Mavis__deep-research_Please_improve__43147106.md) |
| 3 | Mavis | 2026-08-17 | 2 | /team /deep-research  Https:// | [`Mavis__team_deep-research_Https__43195273.md`](sessions/Mavis__team_deep-research_Https__43195273.md) |
| 4 | Mavis | 2026-08-17 | 2 | /team /deep-research  Https:// | [`Mavis__team_deep-research_Https__43195344.md`](sessions/Mavis__team_deep-research_Https__43195344.md) |
| 5 | General | 2026-10-26 | 2 | Distributor sweep DR | [`General__Distributor_sweep_DR__d8d9899c.md`](sessions/General__Distributor_sweep_DR__d8d9899c.md) |
| 6 | General | 2026-10-26 | 2 | Marketplace broker s | [`General__Marketplace_broker_s__1a98914e.md`](sessions/General__Marketplace_broker_s__1a98914e.md) |
| 7 | General | 2026-10-26 | 2 | Datasheet specs Sams | [`General__Datasheet_specs_Sams__80cc6ed8.md`](sessions/General__Datasheet_specs_Sams__80cc6ed8.md) |
| 8 | General | 2026-10-26 | 3 | EU customs tax and F | [`General__EU_customs_tax_and_F__adf77617.md`](sessions/General__EU_customs_tax_and_F__adf77617.md) |
| 9 | General | 2026-10-26 | 2 | Switch OLED 8GB mod  | [`General__Switch_OLED_8GB_mod__1b6223e9.md`](sessions/General__Switch_OLED_8GB_mod__1b6223e9.md) |
| 10 | Mavis | 2026-11-01 | 100 | Reescritura crítica de carta | [`Mavis__Reescritura_crítica_de_carta__45850757.md`](sessions/Mavis__Reescritura_crítica_de_carta__45850757.md) |
| 11 | General | 2026-11-01 | 14 | Adversarial narrativ | [`General__Adversarial_narrativ__e5164d80.md`](sessions/General__Adversarial_narrativ__e5164d80.md) |
| 12 | Verifier | 2026-11-01 | 42 | Mechanical prose aud | [`Verifier__Mechanical_prose_aud__29d9e560.md`](sessions/Verifier__Mechanical_prose_aud__29d9e560.md) |
| 13 | General | 2026-11-01 | 2 | Critico adversarial de la carta | [`General__Critico_adversarial_de_la_carta__45850878.md`](sessions/General__Critico_adversarial_de_la_carta__45850878.md) |
| 14 | Mavis | 2026-11-18 | 100 | Full file | [`Mavis__Full_file__46480247.md`](sessions/Mavis__Full_file__46480247.md) |
| 15 | General | 2026-11-19 | 100 | Auditar harness SEER | [`General__Auditar_harness_SEER__cc01d5d7.md`](sessions/General__Auditar_harness_SEER__cc01d5d7.md) |
| 16 | General | 2026-11-19 | 100 | Auditar hardware y s | [`General__Auditar_hardware_y_s__46e23911.md`](sessions/General__Auditar_hardware_y_s__46e23911.md) |
| 17 | General | 2026-11-19 | 100 | Auditoría adversaria | [`General__Auditoría_adversaria__d41a8f14.md`](sessions/General__Auditoría_adversaria__d41a8f14.md) |
| 18 | General | 2026-11-19 | 100 | Implementar inventar | [`General__Implementar_inventar__6d7ffc8f.md`](sessions/General__Implementar_inventar__6d7ffc8f.md) |
| 19 | General | 2026-11-19 | 100 | Implementar actualiz | [`General__Implementar_actualiz__acaf89cb.md`](sessions/General__Implementar_actualiz__acaf89cb.md) |
| 20 | General | 2026-11-19 | 100 | Implementar analizad | [`General__Implementar_analizad__27a19f3a.md`](sessions/General__Implementar_analizad__27a19f3a.md) |
| 21 | General | 2026-11-19 | 100 | Implementar servidor | [`General__Implementar_servidor__23c1e5eb.md`](sessions/General__Implementar_servidor__23c1e5eb.md) |
| 22 | General | 2026-11-19 | 100 | Implementar extracto | [`General__Implementar_extracto__6cb64e00.md`](sessions/General__Implementar_extracto__6cb64e00.md) |
| 23 | General | 2026-11-19 | 100 | Decompilador DEX en  | [`General__Decompilador_DEX_en__659db26c.md`](sessions/General__Decompilador_DEX_en__659db26c.md) |
| 24 | General | 2026-11-19 | 100 | Firma Rust sin Java | [`General__Firma_Rust_sin_Java__141d9576.md`](sessions/General__Firma_Rust_sin_Java__141d9576.md) |
| 25 | General | 2026-11-19 | 100 | Delegación parasitar | [`General__Delegación_parasitar__8b154d6a.md`](sessions/General__Delegación_parasitar__8b154d6a.md) |
| 26 | General | 2026-11-19 | 100 | Intel de apps y ocul | [`General__Intel_de_apps_y_ocul__275a2232.md`](sessions/General__Intel_de_apps_y_ocul__275a2232.md) |
| 27 | General | 2026-11-19 | 100 | Bus de coordinación  | [`General__Bus_de_coordinación__9cb64913.md`](sessions/General__Bus_de_coordinación__9cb64913.md) |
| 28 | General | 2026-11-19 | 100 | Batería avanzada y m | [`General__Batería_avanzada_y_m__7ac6d702.md`](sessions/General__Batería_avanzada_y_m__7ac6d702.md) |
| 29 | General | 2026-11-19 | 100 | Grabador llamadas y  | [`General__Grabador_llamadas_y__79abadea.md`](sessions/General__Grabador_llamadas_y__79abadea.md) |
| 30 | General | 2026-11-19 | 100 | Rediseño web y API d | [`General__Rediseño_web_y_API_d__e81fade1.md`](sessions/General__Rediseño_web_y_API_d__e81fade1.md) |
| 31 | General | 2026-11-19 | 100 | Planificación conjun | [`General__Planificación_conjun__45e035b9.md`](sessions/General__Planificación_conjun__45e035b9.md) |
| 32 | General | 2026-11-19 | 100 | Auditoría adversaria | [`General__Auditoría_adversaria__7593c4fd.md`](sessions/General__Auditoría_adversaria__7593c4fd.md) |
| 33 | MegaDaemon-RE | 2026-11-21 | 100 | RE profunda Swift Backup + Lucky Patcher + otros | [`MegaDaemon-RE__RE_profunda_Swift_Backup__Lucky_Patcher__otros__46561994.md`](sessions/MegaDaemon-RE__RE_profunda_Swift_Backup__Lucky_Patcher__otros__46561994.md) |
| 34 | MegaDaemon-APK | 2026-11-21 | 100 | Escáner de APK + veredicto de software instalado | [`MegaDaemon-APK__Escáner_de_APK__veredicto_de_software_instalado__46561994.md`](sessions/MegaDaemon-APK__Escáner_de_APK__veredicto_de_software_instalado__46561994.md) |
| 35 | General | 2026-11-21 | 100 | Motor de backup Android nativo + criptografía de formatos | [`General__Motor_de_backup_Android_nativo__criptografía_de___46562016.md`](sessions/General__Motor_de_backup_Android_nativo__criptografía_de___46562016.md) |
| 36 | MegaDaemon-NPU | 2026-11-21 | 100 | Fix ioctl portable Linux/Android (P0 CI) | [`MegaDaemon-NPU__Fix_ioctl_portable_LinuxAndroid_P0_CI__46562376.md`](sessions/MegaDaemon-NPU__Fix_ioctl_portable_LinuxAndroid_P0_CI__46562376.md) |
| 37 | General | 2026-11-21 | 100 | Triaje manual QA y r | [`General__Triaje_manual_QA_y_r__def6d115.md`](sessions/General__Triaje_manual_QA_y_r__def6d115.md) |
| 38 | General | 2026-11-21 | 100 | KDE Connect y arquit | [`General__KDE_Connect_y_arquit__da352c1f.md`](sessions/General__KDE_Connect_y_arquit__da352c1f.md) |
| 39 | General | 2026-11-21 | 100 | Certificados y sedes | [`General__Certificados_y_sedes__371c8b0a.md`](sessions/General__Certificados_y_sedes__371c8b0a.md) |
| 40 | General | 2026-11-21 | 100 | Telefonía SMS OTP an | [`General__Telefonía_SMS_OTP_an__c5a5d49e.md`](sessions/General__Telefonía_SMS_OTP_an__c5a5d49e.md) |
| 41 | General | 2026-11-21 | 100 | Grabaciones: backup  | [`General__Grabaciones_backup__b5801407.md`](sessions/General__Grabaciones_backup__b5801407.md) |
| 42 | General | 2026-11-21 | 100 | Sync chats WhatsApp  | [`General__Sync_chats_WhatsApp__c4418b5a.md`](sessions/General__Sync_chats_WhatsApp__c4418b5a.md) |
| 43 | General | 2026-11-21 | 100 | Gestor de paquetes y | [`General__Gestor_de_paquetes_y__5e7637a4.md`](sessions/General__Gestor_de_paquetes_y__5e7637a4.md) |
| 44 | General | 2026-11-21 | 100 | Go decompiler + Anti | [`General__Go_decompiler__Anti__2c1ce376.md`](sessions/General__Go_decompiler__Anti__2c1ce376.md) |
| 45 | General | 2026-11-21 | 100 | Motor backup nativo  | [`General__Motor_backup_nativo__80f1e15d.md`](sessions/General__Motor_backup_nativo__80f1e15d.md) |
| 46 | General | 2026-11-22 | 100 | Recuperar flujo y ba | [`General__Recuperar_flujo_y_ba__7e718073.md`](sessions/General__Recuperar_flujo_y_ba__7e718073.md) |
| 47 | General | 2026-11-22 | 100 | Inventario masivo de | [`General__Inventario_masivo_de__14f5b117.md`](sessions/General__Inventario_masivo_de__14f5b117.md) |
| 48 | General | 2026-11-22 | 100 | Backup configs certs | [`General__Backup_configs_certs__93e011f6.md`](sessions/General__Backup_configs_certs__93e011f6.md) |
| 49 | General | 2026-11-22 | 95 | Inventario exhaustiv | [`General__Inventario_exhaustiv__d7a4161e.md`](sessions/General__Inventario_exhaustiv__d7a4161e.md) |
| 50 | General | 2026-11-22 | 100 | RE tools Go sobre el | [`General__RE_tools_Go_sobre_el__ec355f10.md`](sessions/General__RE_tools_Go_sobre_el__ec355f10.md) |
| 51 | General | 2026-11-22 | 100 | Ghidra headless sobr | [`General__Ghidra_headless_sobr__74c9d0a0.md`](sessions/General__Ghidra_headless_sobr__74c9d0a0.md) |
| 52 | General | 2026-11-22 | 100 | Instrumentar Antigra | [`General__Instrumentar_Antigra__f9b2d59e.md`](sessions/General__Instrumentar_Antigra__f9b2d59e.md) |
| 53 | General | 2026-11-22 | 60 | Antigravity instrume | [`General__Antigravity_instrume__865ba76b.md`](sessions/General__Antigravity_instrume__865ba76b.md) |
| 54 | General | 2026-11-22 | 100 | RE Go deofuscacion c | [`General__RE_Go_deofuscacion_c__320fe958.md`](sessions/General__RE_Go_deofuscacion_c__320fe958.md) |
| 55 | General | 2026-11-22 | 100 | Backup nativoApps y  | [`General__Backup_nativoApps_y__805fb487.md`](sessions/General__Backup_nativoApps_y__805fb487.md) |
| 56 | General | 2026-11-22 | 74 | Testing profundo LLM | [`General__Testing_profundo_LLM__30dcdadf.md`](sessions/General__Testing_profundo_LLM__30dcdadf.md) |
| 57 | General | 2026-11-22 | 21 | Titanium Backup y mi | [`General__Titanium_Backup_y_mi__e783de3c.md`](sessions/General__Titanium_Backup_y_mi__e783de3c.md) |
| 58 | General | 2026-11-22 | 82 | Ocultacion root y Ma | [`General__Ocultacion_root_y_Ma__50cc62d8.md`](sessions/General__Ocultacion_root_y_Ma__50cc62d8.md) |
| 59 | General | 2026-11-22 | 59 | Kernel y NetHunter | [`General__Kernel_y_NetHunter__bace2e54.md`](sessions/General__Kernel_y_NetHunter__bace2e54.md) |
| 60 | General | 2026-11-22 | 33 | Testing profundo LLM | [`General__Testing_profundo_LLM__e1f54302.md`](sessions/General__Testing_profundo_LLM__e1f54302.md) |
| 61 | General | 2026-11-22 | 17 | RE Go completa | [`General__RE_Go_completa__cb36d262.md`](sessions/General__RE_Go_completa__cb36d262.md) |
| 62 | General | 2026-11-22 | 46 | Backup nativo v2 | [`General__Backup_nativo_v2__73194783.md`](sessions/General__Backup_nativo_v2__73194783.md) |
| 63 | General | 2026-11-22 | 100 | Titanium Backup y mi | [`General__Titanium_Backup_y_mi__7d361b44.md`](sessions/General__Titanium_Backup_y_mi__7d361b44.md) |
| 64 | General | 2026-11-22 | 100 | Ocultacion root y Ma | [`General__Ocultacion_root_y_Ma__4508dd68.md`](sessions/General__Ocultacion_root_y_Ma__4508dd68.md) |
| 65 | General | 2026-11-22 | 100 | Kernel y NetHunter | [`General__Kernel_y_NetHunter__3bf90992.md`](sessions/General__Kernel_y_NetHunter__3bf90992.md) |
| 66 | General | 2026-11-22 | 26 | Software instalado y | [`General__Software_instalado_y__cf791e13.md`](sessions/General__Software_instalado_y__cf791e13.md) |
| 67 | General | 2026-11-22 | 83 | Sistema agentico del | [`General__Sistema_agentico_del__f44fc010.md`](sessions/General__Sistema_agentico_del__f44fc010.md) |
| 68 | General | 2026-11-22 | 100 | Continuar Antigravit | [`General__Continuar_Antigravit__6d0c5d7f.md`](sessions/General__Continuar_Antigravit__6d0c5d7f.md) |
| 69 | General | 2026-11-22 | 79 | Continuar RE Go comp | [`General__Continuar_RE_Go_comp__b025ccbe.md`](sessions/General__Continuar_RE_Go_comp__b025ccbe.md) |
| 70 | General | 2026-11-22 | 100 | Continuar backup nat | [`General__Continuar_backup_nat__951f01dd.md`](sessions/General__Continuar_backup_nat__951f01dd.md) |
| 71 | General | 2026-11-22 | 100 | Continuar Titanium y | [`General__Continuar_Titanium_y__1f302ff8.md`](sessions/General__Continuar_Titanium_y__1f302ff8.md) |
| 72 | General | 2026-11-22 | 86 | Continuar root y Mag | [`General__Continuar_root_y_Mag__ba124624.md`](sessions/General__Continuar_root_y_Mag__ba124624.md) |
| 73 | General | 2026-11-22 | 100 | RE agresivo unificad | [`General__RE_agresivo_unificad__a183be48.md`](sessions/General__RE_agresivo_unificad__a183be48.md) |
| 74 | General | 2026-11-22 | 100 | Asimilar Antigravity | [`General__Asimilar_Antigravity__aac50031.md`](sessions/General__Asimilar_Antigravity__aac50031.md) |
| 75 | General | 2026-11-22 | 100 | Backup nativo v2 rec | [`General__Backup_nativo_v2_rec__db28f9cf.md`](sessions/General__Backup_nativo_v2_rec__db28f9cf.md) |
| 76 | General | 2026-11-22 | 100 | Testing profundo LLM | [`General__Testing_profundo_LLM__60c4eee7.md`](sessions/General__Testing_profundo_LLM__60c4eee7.md) |
| 77 | General | 2026-11-22 | 100 | RE agresivo sin comp | [`General__RE_agresivo_sin_comp__c43310af.md`](sessions/General__RE_agresivo_sin_comp__c43310af.md) |
| 78 | General | 2026-11-23 | 100 | Fix s14 y bins rojos | [`General__Fix_s14_y_bins_rojos__f6790cf7.md`](sessions/General__Fix_s14_y_bins_rojos__f6790cf7.md) |
| 79 | General | 2026-11-23 | 100 | Generador de código  | [`General__Generador_de_código__3dd807e4.md`](sessions/General__Generador_de_código__3dd807e4.md) |
| 80 | General | 2026-11-23 | 100 | Cazador de bugs y ca | [`General__Cazador_de_bugs_y_ca__34f74ed7.md`](sessions/General__Cazador_de_bugs_y_ca__34f74ed7.md) |
| 81 | General | 2026-11-23 | 100 | Regenerar protobuf d | [`General__Regenerar_protobuf_d__90e7913a.md`](sessions/General__Regenerar_protobuf_d__90e7913a.md) |
| 82 | General | 2026-11-23 | 100 | Inyección Frida nati | [`General__Inyección_Frida_nati__058e2183.md`](sessions/General__Inyección_Frida_nati__058e2183.md) |
| 83 | General | 2026-11-23 | 100 | Payload Frida con fr | [`General__Payload_Frida_con_fr__96fcd216.md`](sessions/General__Payload_Frida_con_fr__96fcd216.md) |
| 84 | General | 2026-11-23 | 100 | Agente dedicado CI r | [`General__Agente_dedicado_CI_r__eba88e7a.md`](sessions/General__Agente_dedicado_CI_r__eba88e7a.md) |
| 85 | General | 2026-11-23 | 100 | Rescate generador Go | [`General__Rescate_generador_Go__c075e921.md`](sessions/General__Rescate_generador_Go__c075e921.md) |
| 86 | General | 2026-11-23 | 100 | Rescate payload frid | [`General__Rescate_payload_frid__0737c26c.md`](sessions/General__Rescate_payload_frid__0737c26c.md) |
| 87 | General | 2026-11-23 | 100 | Rescate regenerador  | [`General__Rescate_regenerador__093a48ff.md`](sessions/General__Rescate_regenerador__093a48ff.md) |
| 88 | General | 2026-11-23 | 100 | MITM SQLite y proxy  | [`General__MITM_SQLite_y_proxy__58caace9.md`](sessions/General__MITM_SQLite_y_proxy__58caace9.md) |
| 89 | General | 2026-11-23 | 100 | Auditar ramas y rota | [`General__Auditar_ramas_y_rota__7ee40e68.md`](sessions/General__Auditar_ramas_y_rota__7ee40e68.md) |
| 90 | General | 2026-11-26 | 90 | Índice consultable d | [`General__Índice_consultable_d__1f969aa6.md`](sessions/General__Índice_consultable_d__1f969aa6.md) |
| 91 | General | 2026-11-26 | 30 | Trazar decisión de l | [`General__Trazar_decisión_de_l__9132b984.md`](sessions/General__Trazar_decisión_de_l__9132b984.md) |
