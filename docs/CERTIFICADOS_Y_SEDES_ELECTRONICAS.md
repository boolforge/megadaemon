# Certificados y sedes electrónicas: qué se puede automatizar y qué no

> **Ámbito**: investigación para el harness MegaDaemon (`rust-core`) sobre los
> sistemas con los que el usuario quiere automatizar **sus propios** trámites:
> SALUD Andalucía (historia clínica electrónica), Receta Electrónica Andalucía,
> Sede Electrónica de la Junta de Andalucía, Sede Judicial Electrónica y
> Seguridad Social — y sobre los **certificados** que hacen falta para hablar
> con ellos.
>
> **Fecha de la consulta**: 2026-11 (fuentes consultadas en esa fecha).
> **Regla de oro de este documento**: *no se automatiza nada que sea una
> decisión clínica, una dispensaciónPharmacéutica ni una firma en nombre del
> usuario*. Eso va marcado como «requiere presencia/consentimiento» en cada
> tabla y está desarrollado en `docs/CERT_SYNC_EN_NODOS.md`.

---

## 0. Cómo leer las fuentes (honestidad de la verificación)

Cada fuente de este documento está etiquetada con **cómo** se obtuvo:

| Etiqueta | Significado |
|---|---|
| `FETCH` | La URL se **abrió directamente** con el recuperador de páginas (`web_fetch`) y se cita el texto literal de la página. |
| `ÍNDICE` | La URL oficial es real y su texto literal llegó **íntegro** vía el índice del buscador, pero el recuperador directo agotó el tiempo de espera (los dominios `sspa.juntadeandalucia.es` y `juntadeandalucia.es` rechazan/expanden el fetch directo). Se cita el texto literal devuelto por el índice. |
| `NO VERIFICADO` | **No** he podido comprobarlo con una fuente oficial. No se afirma como hecho: se marca. |

Lo que **no** aparece en este documento: nombres de trámites, URLs o requisitos
inventados. Si algo no estaba en la fuente, está marcado `NO VERIFICADO`.

---

## 1. Resumen ejecutivo (la tabla que hay que leer primero)

| Sistema | URL oficial | Acceso mínimo | ¿Permite mTLS? | Qué automatizar con seguridad | Qué NO automatizar | Fuente |
|---|---|---|---|---|---|---|
| **SALUD Andalucía** (app) / **ClicSalud+** (web) — historia clínica electrónica | `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/ciudadania/app-salud-andalucia` · `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/ciudadania/clicsalud/salud` | **Certificado digital, DNIe o Cl@ve con registro avanzado** para datos de salud; sólo *datos personales* con identificación simple | Sí (certificado/DNIe) — Cl@ve no usa mTLS | Descargar *informes clínicos*, resultados de analíticas y de imagen, y la medicación que el propio paciente ya tiene prescrita; avisar al usuario cuando hay novedad | **NO** introducir ni modificar datos clínicos; **NO** "recomendar" medicación; **NO** consentir por el usuario accesos a datos de salud (art. 9 RGPD) | `ÍNDICE` (texto literal de la página oficial del SAS) |
| **Receta Electrónica Andalucía** ("Receta XXI", módulo DIRAYA + MCD) | `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/ciudadania/farmacia-y-prestaciones/informacion-la-ciudadania-sobre-el-uso-de-medicamentos/receta-electronica-preguntas-frecuentes` | Paciente: **tarjeta sanitaria o DNIe** en la farmacia; consulta del tratamiento: ClicSalud+/app con certificado, DNIe o Cl@ve | No aplica al paciente (el dispensing lo hace el farmacéutico con la tarjeta/DNIe del paciente) | **Consultar** el tratamiento activo, las dispensaciones ya hechas y la fecha de caducidad de cada envase | **NO prescribir** (sólo el profesional sanitario en DIRAYA); **NO dispensar** (sólo el farmacéutico acreditado en el MCD); **NO** automatizar la retirada física del medicamento | `ÍNDICE` + manual oficial Receta XXI (PDF) `ÍNDICE` |
| **Sede Electrónica General de la Junta de Andalucía** | `https://www.juntadeandalucia.es/servicios/sede.html` (portal) · `https://sede.juntadeandalucia.es/` | **Navegar sin identificarse**; para tramitar: certificado electrónico admitido **o** Cl@ve; el **Registro Electrónico exige certificado electrónico** | Sí (certificado de cliente) | Consultar el **catálogo de procedimientos**, descargar formularios y solicitudes, y leer la **Carpeta Ciudadana** (documentos presentados, estado de expedientes) y las **notificaciones electrónicas** | **NO** presentar escritos ni firmar en nombre del usuario sin que él lo confirme; **NO** usar la firma electrónica cualificada de forma unattended | `ÍNDICE` (FAQ oficial de la Sede + Decreto 622/2019, BOJA) |
| **Sede Judicial Electrónica (SJE)** — Justicia | `https://sedejudicial.justicia.es/` | **Acceso público sin identificación**; para los trámites: **DNIe/certificado electrónico, Cl@ve PIN o Cl@ve Permanente** | Sí (DNIe/certificado) | Consultar el estado de un **expediente propio**, descargar notificaciones y avisos, y leer el catálogo de servicios | **NO** presentar escritos, **NO** firmar documentos judiciales, **NO** usar LexNET (exige certificado válido y es de-El profesional de la justicia) | `FETCH` (`firma-y-certificados-electronicos-admitidos`) + `ÍNDICE` (FAQ oficial) |
| **Seguridad Social** (Sede Electrónica) | `https://sede.seg-social.gob.es/` | **Certificado electrónico cualificado admitido**, **Cl@ve Móvil** o **Cl@ve Permanente**; para "Tu Seguridad Social" hace falta **registro avanzado** en Cl@ve o certificado digital admitido | Sí (certificado de cliente) | Descargar el **informe de vida laboral**, certificados de pensión, estado de expedientes, notificaciones telemáticas y documentos de la Carpeta Ciudadana | **NO** solicitar prestaciones en nombre del usuario, **NO** firmar solicitudes de Todo lo que implique firma cualificada | `FETCH` (requisitos por tipo de acceso) + `ÍNDICE` (FAQ oficial, Carpeta Ciudadana) |
| **Cl@ve** (plataforma) | `https://clave.gob.es/clave/que-es` | Registro: presencial, por internet con **certificado o DNIe**, o por **vídeo**; luego **Cl@ve Móvil**, **Cl@ve Permanente** o **Cl@ve PIN** | No (la identificación es por contraseña concertada + segundo factor, no por certificado de cliente) | Reutilizar **Cl@ve Móvil** desde un nodo (push/QR al móvil del usuario) sin guardar credenciales; **Cl@ve Permanente** sólo con su contraseña, protegida como un secreto más | **NO** guardar la contraseña de Cl@ve Permanente en claro ni automatizar el PIN como si fuera una credencial reutilizable | `ÍNDICE` (`clave.gob.es`) + `ÍNDICE` (`administracion.gob.es`) |
| **Certificado de persona física / "Certificado de Ciudadano" (FNMT-CERES)** | `https://www.sede.fnmt.gob.es/certificados/persona-fisica` | Solicitud: vídeo-identificación, acreditación presencial, **DNIe** o **dispositivo móvil** | Sí (es el certificado de cliente en TLS) | Llevar el **certificado público** a los nodos; usar el `.p12`/`.pfx` **cifrado con contraseña** para el handshake | **NO** dejar la clave privada en claro en ningún nodo; **NO** automatizar la **reemisión** (el FNMT avisa: pedir otro por la app **revoca** el anterior) | `FETCH` |
| **Certificado de persona física "de la AGE"** | — | — | — | — | — | **`NO VERIFICADO`**: no he encontrado un producto oficial con ese nombre exacto. Ver §6.3. |

> **Nota sobre «mTLS»**: las páginas oficiales que he abierto hablan de
> «certificado electrónico cualificado admitido», de «instalar el certificado
> en el navegador» y de «instalar el software criptográfico DNIe». **Ninguna
> usa la sigla «mTLS»**: la columna refleja el mecanismo técnico real
> (autenticación TLS de cliente / *client certificate*), pero la **sigla** es
> inferencia técnica mía, no una cita textual.

---

## 2. SALUD Andalucía: historia clínica electrónica en el móvil

### 2.1 Qué es exactamente (el nombre real)

El usuario habló de «Respuesta Salud». Los nombres **oficiales** que he
verificado son:

* **App «Salud Andalucía»** — «la aplicación móvil de referencia institucional
  para todas las personas usuarias del Servicio Andaluz de Salud» … «permite la
  realización de los trámites más habituales, como la solicitud de cita, y
  también la consulta de la historia clínica electrónica».
  (`ÍNDICE`, página oficial del SAS)
* **«Salud Responde»** — la app histórica de cita en atención primaria; hoy es
  una de las apps que **«Salud Andalucía»** agrega junto a «Dona Sangre»,
  «Mi prescripción» y «Vacunas» (`ÍNDICE`, nota de prensa oficial de la Junta).
* **ClicSalud+** — el portal **web** equivalente (historia clínica, analíticas,
  imagen, medicación, trámites).
  (`ÍNDICE`, página oficial del SAS)

⇒ En el código y en la documentación hay que usar **«Salud Andalucía»** (app) y
**«ClicSalud+»** (web). «Respuesta Salud» no es el nombre de la app: es una
confusión con «Salud Responde», que además hoy está **integrada** en «Salud
Andalucía». `NO VERIFICADO`: no he encontrado ninguna app oficial llamada
«Respuesta Salud».

### 2.2 Qué se necesita para entrar

De la página oficial de la app (`ÍNDICE`, cita literal):

> «Las opciones principales, que darán acceso a todos los servicios, son
> utilizar un certificado digital, disponer de un registro avanzado de Cl@ve o
> bien usar un código QR que se obtiene a partir de uno de esos sistemas.»

> «Por ejemplo, para acceder a información de sus informes clínicos no es
> suficiente haberse identificado a través de alguna de las opciones de Datos
> Personales, por lo que le pedirá que utilice Certificado Digital o Cl@ve.»

Y de la ayuda de Cl@ve para la app (`ÍNDICE`, cita literal):

> «Para consultar datos de salud se requiere Cl@ve con nivel avanzado de
> registro (obtenido de forma presencial o con certificado electrónico).»
> «No podrá acceder a su información de salud si realizó un registro de nivel
> básico en Cl@ve.»

Niveles de identificación en la app (`ÍNDICE`):

| Opción | Qué permite |
|---|---|
| Datos personales (básica) | Datos de contacto de la BD de usuarios del SSPA |
| Certificado digital | Acceso a **datos de salud** y a todos los servicios |
| Cl@ve (registro avanzado) | Acceso a **datos de salud** y a todos los servicios |
| QR (derivado de certificado o Cl@ve) | Igual que el sistema de origen |
| eIDas | *"Identificación para ciudadanos de la unión Europea"* (`NO VERIFICADO`: no se detalla el alcance) |

### 2.3 Qué se puede consultar (y es dato de salud)

De la página oficial de "Salud" en ClicSalud+ (`ÍNDICE`, cita literal):

> «En ClicSalud+ y en la App Salud Andalucía puede consultar los datos de
> salud más relevantes registrados en su historia clínica electrónica,
> relacionarse con los profesionales que le atienden, e introducir
> información para el seguimiento de su salud. Los datos sobre salud están
> especialmente protegidos, por lo que se requiere identificarse mediante
> certificado digital, DNie o Cl@ve.»

Y sobre el catálogo de la app (`ÍNDICE`):

> «Salud: consultar los datos de salud más relevantes (informes clínicos,
> medicación, resultados de pruebas analíticas, pruebas de imagen...)»

Detalle fino (`ÍNDICE`): los informes clínicos que se pueden consultar e
imprimir son los de **hospitalización, consultas externas y urgencias
hospitalarias**; los resultados de analíticas son los que «consten en su
historia clínica electrónica».

### 2.4 El dato de salud: categoría especial (RGPD art. 9)

Esto es la parte que **no** se puede automatizar a la ligera:

* Los datos de salud son **categoría especial** del art. 9 del RGPD: su
  tratamiento está **prohibido** salvo que concurra una de las excepciones del
  art. 9.2 (consentimiento explícito del interesado, ejecución de un contrato
  con el interesado, obligaciones legales, etc.).
* En el caso del usuario final que consulta **su propia** historia clínica, la
  base jurídica habitual es el **consentimiento explícito** (art. 9.2.a) y la
  herencia del derecho de acceso del interesado (arts. 12–15 RGPD), no la
 la simple ejecución de una orden del agente.
* Consecuencia práctica para el harness: un nodo **no** puede responder "¿qué
  tienes en la analítica?" sin que:
  1. el usuario se haya autenticado **él mismo** (certificado/DNIe/Cl@ve
     avanzado) en **esa** sesión, y
  2. el nodo tenga una **base jurídica** registrada para tratar categoría
     especial, y
  3. exista **traza de auditoría** de quién pidió qué.
* `NO VERIFICADO`: las condiciones exactas de la relación de responsabilidad
  (p. ej. si el nodo es un «encargado de tratamiento» del usuario o un
  autónomo) y la base jurídica concreta de cada SAD (SAS) no están en las páginas
  que he abierto: hay que leer el aviso de privacidad de la app y el RGPD del SAS
  para eso.

### 2.5 Qué habilita el usuario (y qué hay que respetar)

| Acción del usuario | Efecto | Automatizable |
|---|---|---|
| Identificarse con certificado/DNIe/Cl@ve avanzado | Acceso a sus datos de salud | Sí, **como paso explícito del usuario** (nunca "en nombre de") |
| Suscribirse a **AviSAS** (avisos personalizados de asistencia sanitaria) | Recibe un aviso cuando se le asigna o anula una cita de consulta o prueba diagnóstica, y recordatorios de citas | El aviso **lo manda el SAS**; el nodo no puede "activar" la suscripción por su cuenta (`ÍNDICE`: «Para recibir estos avisos, es necesario suscribirse al sistema») |
| Usar ClicSalud+ desde la app | Mismos servicios que la web | Sí, **con la sesión del usuario** |

---

## 3. Receta Electrónica Andalucía (SAAS) — qué automatizar y qué no

### 3.1 Cómo funciona (hechos)

De la FAQ oficial (`ÍNDICE`, citas literales):

> «Cuando el médico realiza la prescripción de un medicamento o producto
> sanitario a un paciente a través de la aplicación informática de Historia de
> Salud Digital, denominada DIRAYA (para lo que se requiere que el paciente
> entregue la tarjeta sanitaria a su médico), determinados datos de dicha
> prescripción son grabados en un Módulo Central de Dispensación (MCD).»

> «Para la dispensación de los productos prescritos por el médico, el paciente
> se dirigirá a una oficina de farmacia. En dicho establecimiento debe
> presentar su tarjeta sanitaria como medio de autentificación y como
> dispositivo que servirá al farmacéutico, debidamente acreditado por el
> Servicio Andaluz de Salud, para acceder, vía web, al ya citado Módulo
> Central de Dispensación (MCD).»

> «En estos momentos también es posible la dispensación utilizando el DNI
> electrónico.»

El manual oficial de Receta XXI (`ÍNDICE`, PDF de la Subdirección de Farmacia y
Prestaciones) añade:

> «Para realizar una prescripción mediante el sistema de receta electrónica ya
> no es necesaria la introducción de la tarjeta sanitaria del paciente, pero sí
> es necesaria para la dispensación (también se puede dispensar aportando el
> DNI electrónico).»

Y sobre el módulo de dispensación (`ÍNDICE`):

> «Receta XXI Dispensación es una aplicación incluida dentro del proyecto
> Diraya que facilita al farmacéutico visualizar las prescripciones que ha
> realizado el médico y registrar las dispensaciones efectuadas a lo largo del
> tratamiento.»

### 3.2 La frontera (esto es lo importante)

| Acción | ¿Quién la hace? | Automatizable por el nodo |
|---|---|---|
| **Prescribir** (elegir fármaco, dosis, pauta, duración) | Médico/enfermero/odontólogo en DIRAYA | **NO**. Es una decisión clínico-terapéutica: requiere criterio profesional y, en su caso, la consulta al paciente. `NO VERIFICADO`: si la prescripción electrónica exige firma profesional cualificada en todos los casos (el manual no lo detalla para todos los perfiles). |
| **Dispensar** (entregar el envase y registrarlo en el MCD) | Farmacéutico acreditado, con la tarjeta sanitaria o el DNIe del paciente | **NO**. El MCD autentica a la persona con su tarjeta/DNIe **en la oficina de farmacia**; automatizarlo desde un nodo sería suplantar al paciente en la farmacia y saltarse al profesional. |
| **Consultar** el tratamiento activo, las dispensaciones ya realizadas y su caducidad | El paciente, en ClicSalud+ «Salud → medicación» o en la app «Mi prescripción» (con certificado, DNIe o clave) | **Sí**, en modo **sólo lectura**, con la sesión del usuario. El folleto oficial (`ÍNDICE`) lo dice: «En la Aplicación de Salud Andalucía…», «Puedes consultar las dispensaciones que has tenido de cada producto y en qué farmacia se han realizado». |
| **Avisar** al paciente de que le toca retirar un envase o de que le queda poco | El SAS (la app ya avisa: «Cuando el tratamiento está activo… el siguiente puedes recogerlo 10 días antes de que se te acabe el anterior») | **Sí**, como **recordatorio al usuario** (no como dispensación). |

> **Por qué esta frontera**: la receta electrónica es *uso diario de la salud*.
> Un nodo que "prescribe" o "dispensa" está firmando decisiones sanitarias en
> nombre de un profesional que no ha intervenido, y además está firmando con
> la identidad del paciente. El valor que sí se puede automatizar es
> **informativo y preparatorio**: que el paciente sepa qué tiene que recoger,
> dónde y cuándo, sin tocar la dispensación.

---

## 4. Sede Electrónica de la Junta de Andalucía

### 4.1 Niveles de acceso (lo que admiten)

De la página oficial «Requisitos de acceso» (`ÍNDICE`, cita literal):

> «Para navegar, obtener información sobre un trámite o un servicio, y/o
> descargar documentos y solicitudes cumplimentadas, basta con que dispongas
> de un dispositivo electrónico con acceso a internet, con sistema operativo,
> navegador y lector de documentos PDF estándares. Si lo que quieres es
> iniciar un trámite o consultar información particular necesitas disponer de
> un sistema de identificación y firma admitidos por la Junta de Andalucía.»

De «Sistemas de identificación y firma» (`ÍNDICE`, cita literal):

> «Los sistemas de identificación y firma válidos en la Administración de la
> Junta de Andalucía serán **los basados en certificados electrónicos
> cualificados** conforme a la mencionada Norma Técnica, y aquellos **no basados
> en certificados** según lo dispuesto en los Anexos II y III del citado
> Decreto 622/2019.»

> «Los sistemas más comunes de identificación y firma para la realización de
> trámites en esta Sede son el certificado electrónico y Cl@ve.»

Y el **Decreto 622/2019** (BOJA núm. 250 de 31/12/2019, `ÍNDICE`) concreta los
niveles:

* **Anexo II (identificación, sin certificado)**: «Sistemas de clave concertada
  existentes en la plataforma Cl@ve conformes con el **nivel mínimo sustancial**
  según el Reglamento (UE) N.º 910/2014» + «sistemas de identificación biométrica
  y reconocimiento facial de la Carpeta Ciudadana, de utilización voluntaria».
* **Anexo III (firma, sin certificado)**: firma para **sistemas de categoría
  básica**; y **firma manuscrita digitalizada** en comparecencia presencial.

⇒ Traducción a niveles operativos:

| Nivel | Identificación | Para qué sirve |
|---|---|---|
| 0 · Anónimo | ninguna | Navegar, catálogo, descargar formularios y solicitudes cumplimentadas |
| 1 · Cl@ve (nivel mínimo sustancial) | usuario+contraseña concertada | Identificación en servicios que no exigen firma avanzada |
| 2 · Cl@ve / biometría de la Carpeta Ciudadana | segundo factor / biometría | Igual que 1 + servicios con más datos |
| 3 · Certificado cualificado o DNIe | certificado de cliente | **Firma avanzada**: presentar escritos, firmar, registrar |

### 4.2 Qué se puede automatizar y qué no

De la FAQ oficial (`ÍNDICE`, citas literales):

> «El único requisito para utilizar el Registro Electrónico es disponer de un
> certificado electrónico expedido por alguno de los proveedores de servicios de
> certificación electrónica.»

> «Para poder acceder [a la Carpeta Ciudadana] es preciso que te autentiques
> mediante certificado digital o Cl@ve.»

> «¿Puedo realizar un trámite sin identificación digital? **Sí.** Puedes
> realizar todo el proceso de tramitación online y, cuando llegues al paso de
> firma, imprimir la solicitud y presentarla de forma presencial en las Oficinas
> de Asistencia en Materia de Registro, siempre y cuando el procedimiento no sea
> exclusivamente telemático.»

| Acción | Nivel | Automatizable |
|---|---|---|
| Consultar el catálogo de procedimientos y descargar formularios | 0 | **Sí** |
| Consultar la **Carpeta Ciudadana** (documentos presentados, estado de expedientes) | 1–3 | **Sí**, en sólo lectura |
| Descargar notificaciones electrónicas | 1–3 | **Sí**, en sólo lectura |
| Presentar un escrito en el **Registro Electrónico** | 3 (certificado) | **Con el usuario delante**: el nodo puede *preparar* el escrito y los anexos, pero la firma la hace el usuario (o exige su confirmación explícita y una traza) |
| Tramitar "en línea" (solicita y tramita) | 3 | **No** de forma desatendida: es el procedimiento entero con firma |

`NO VERIFICADO`: la lista completa de trámites "exclusivamente telemáticos" de la
Junta (que no admiten el trámite en papel) **no** está en las páginas abiertas:
hay que mirarla en el catálogo de procedimientos, trámite a trámite.

---

## 5. Sede Judicial Electrónica (SJE) y Justicia

### 5.1 Acceso (verificado con fetch directo)

De `https://sedejudicial.justicia.es/firma-y-certificados-electronicos-admitidos`
(`FETCH`, citas literales):

> «Actualmente en la Sede Judicial Electrónica se puede realizar los siguientes
> tipos de autenticación bajo el uso del sistema Cl@ve:
> - DNIe / Certificado electrónico.
> - Cl@ve PIN.
> - Cl@ve permanente.»

> «No obstante, Cl@ve también permite realizar una identificación mediante el
> uso de DNIe o certificado electrónico sin necesidad de realizar ningún
> registro previo.»

Y del Ministerio de Justicia (`ÍNDICE`, citas literales):

> «Se trata de un portal público accesible para toda la ciudadanía sin necesidad
> de autorización previa. Para el acceso a los servicios telemáticos de
> comunicación con la administración de justicia, se puede realizar la
> autenticación por medio de cualquiera de los sistemas habilitado por la
> Plataforma Cl@ve: Certificado digital (DNIe), Cl@ve PIN o Cl@ve Permanente.»

De la FAQ de la SJE (`ÍNDICE`, citas literales):

> «Los ciudadanos accederán a la página de inicio de las Sedes sin necesidad de
> identificación, pero siempre se exigirá la autenticación del usuario para
> acceder a los trámites electrónicos.»

> «¿Cómo puedo entrar en mi Área Personal? Mediante autenticación con DNI
> electrónico o certificado electrónico. […] También podrá acceder mediante
> cl@ve […] con sus mecanismos de autenticación permitidos.»

> «¿Cómo puedo acceder a LexNET desde la Sede Judicial Electrónica? […]
> Necesitará estar en posesión de un certificado válido para poder acceder.»

> «¿Por qué aparece un mensaje relacionado con el certificado de seguridad de la
> Sede? […] el sitio utiliza un certificado usado por el protocolo TLS para
> cifrar las comunicaciones […]»

### 5.2 Qué automatizar / qué no

| Acción | Automatizable | Nota |
|---|---|---|
| Consultar el estado de un **expediente propio** | **Sí**, con la sesión del usuario | Es la consulta de datos propios: el derecho de acceso del interesado |
| Descargar notificaciones y avisos de la SJE | **Sí**, sólo lectura | |
| Consultar el catálogo de servicios | **Sí**, sin identificarse | |
| Presentar escritos / **firmar** documentos judiciales | **No** | Firma en nombre del usuario: la firma avanzada es un acto personal con efectos jurídicos |
| Acceder a **LexNET** | **No** | Exige certificado válido y es la herramienta de los profesionales de la justicia; además el nodo no es el destinatario real de las notificaciones |
| Actuar como **representante/apoderado** | `NO VERIFICADO` | No he encontrado la página oficial de la SJE que detalle el régimen de representación en la SJE |

> **"Consejo de_POWER"**: la ambigüedad del enunciado del usuario la resuelvo
> como el **CGPJ** (organismo que gestiona la Sede Judicial Electrónica) y el
> **Ministerio de la Presidencia, Justicia y Relaciones con las Cortes**. Lo
> que he verificado es la SJE (`sedejudicial.justicia.es`) y la ficha oficial en
> `administraciondejusticia.gob.es`. `NO VERIFICADO`: no hay ninguna fuente
> oficial con el literal "Consejo de_POWER"; si el usuario se refería a otra
> cosa, esta tabla no la cubre.

---

## 6. Seguridad Social

### 6.1 Acceso (verificado con fetch directo)

De `https://sede.seg-social.gob.es/.../requisitos%20por%20tipo%20de%20acceso/`
(`FETCH`, el servidor devolvió la versión inglesa de la misma página; cito el
original que devolvió):

> «To access the electronic services that offer this authentication method, you
> will need to have a qualified electronic certificate recognised by Social
> Security.» (Certificado electrónico)

> «Access with a qualified digital certificate via the Cl@ve platform does not
> require previous registration.»

> «Cl@ve Mobile: […] The system offers the novelty of allowing a QR code to be
> scanned that avoids the need for users to enter their identification data on
> the access screen. If the QR code can't be scanned, the authentication request
> that will be sent to the Cl@ve APP can be confirmed.»

> «Cl@ve Permanent is a long-term (but not unlimited) password system for
> regular users. Corresponds to the system accessed with a username and
> password. This will also be the system that enables access to a signature in
> the cloud.»

> «It should also be noted that certain services of the Social Security
> Administration Office may require an advanced cl@ve registration. In such
> cases, online registration is not sufficient.»

De la FAQ oficial (`ÍNDICE`, citas literales):

> «Según el perfil, ciudadano, empresa y en función del régimen de afiliación a
> la Seguridad Social, se tendrá acceso a la presentación de documentación por
> Registro Electrónico, recepción de Notificaciones Telemáticas, obtención de
> Certificados electrónicos y otros servicios que antes requerían presencia
> física.»

> «Para entrar en Tu Seguridad Social, el usuario cl@ve solo es válido si el
> registro ha sido de nivel avanzado.»

> «Además, también puede obtener, imprimir y/o consultar este informe en el
> mismo momento de su petición, a través del acceso **con certificado
> digital**, **vía SMS** […] y a través del acceso **Usuario/contraseña -
> Cl@ve permanente** […]»

> «Si accede **sin certificado digital** podrá cumplimentar un formulario de
> solicitud de modo que el informe le será remitido por correo postal.»

De la **Carpeta Ciudadana** de la Seguridad Social (`ÍNDICE`, cita literal):

> «You will need a secure digital identity, either through Cl@ve (Cl@ve Mobile
> or Permanent Cl@ve), DNIe or any recognised digital certificate.»

De la página de **certificados admitidos** de la Seguridad Social (`ÍNDICE`),
las autoridades de certificación admitidas incluyen: «Dirección General de la
Policía y de la Guardia Civil – Cuerpo Nacional de Policía (DNI electrónico)»,
«Fábrica Nacional de Moneda y Timbre-Real Casa de la Moneda (FNMT-RCM)»,
«Ministerio de Inclusión, Seguridad Social y Migraciones», IZENPE, ACCV,
CATCert, ANCERT, Camerfirma, SIA, etc.

### 6.2 Qué automatizar / qué no

| Acción | Acceso | Automatizable |
|---|---|---|
| **Informe de vida laboral** (PDF) y otros informes | certificado, SMS o Cl@ve Permanente | **Sí**, sólo lectura/descarga |
| Consultar periodos de cotización, pensión y sus cuantías, certificados de prestación | Cl@ve avanzado / certificado | **Sí**, sólo lectura |
| Notificaciones telemáticas, estado de expedientes ("Mis Expedientes") | Cl@ve / DNIe / certificado | **Sí**, sólo lectura |
| Registro Electrónico: presentar solicitudes y documentación | Cl@ve / certificado | **Preparar** sí; **firmar y presentar**, con confirmación explícita del usuario |
| Altas/bajas de autónomo, solicitudes de prestación, representación de terceros | Cl@ve avanzado / certificado de representante | **No** desatendido: implican datos y efectos jurídicos sobre la persona del usuario |
| Informes que sólo se remiten por correo postal (sin certificado) | — | La Seguridad Social manda el informe al domicilio si se accede sin certificado (`ÍNDICE`) |

---

## 7. Cl@ve: las tres vías y cuál puede reutilizar un nodo

De `https://clave.gob.es/clave/que-es` (`ÍNDICE`, citas literales):

> «Cl@ve es un sistema orientado a unificar y simplificar el acceso electrónico
> de los ciudadanos a los servicios públicos. […] Es una plataforma común para
> la identificación, autenticación y firma electrónica […]. Además de
> posibilitar el acceso a trámites electrónicos a quien no dispone de
> certificado electrónico, el sistema Cl@ve aporta un uso sencillo y seguro,
> escaneando un QR, confirmando una petición de autenticación o a través de
> una contraseña permanente.»

> «Cl@ve Móvil: te permite identificarte sin claves ni contraseñas, simplemente
> escaneando un QR o confirmando la petición que te llegará al móvil.»

> «Cl@ve Permanente: orientada a los accesos habituales, y al uso de la firma en
> la nube, con contraseña de validez duradera en el tiempo, pero no ilimitada.
> Es un sistema de acceso mediante usuario y contraseña, reforzado con claves de
> un solo uso remitido por SMS para ciertos servicios de nivel superior.»

De `administracion.gob.es` (`ÍNDICE`, cita literal):

> «Cl@ve Móvil: sistema de autenticación a través de notificación recibida en
> el móvil, sin necesidad de recordar ninguna contraseña.
> - Cl@ve permanente: sistema de contraseña de validez duradera en el tiempo
> (aunque no ilimitada), orientado a usuarios **habituales**.
> - Cl@ve PIN: sistema de contraseña de validez muy limitada en el tiempo,
> orientado a usuarios que acceden con **poca frecuencia** a los servicios.»

Y sobre el registro (`ÍNDICE`, cita literal):

> «Para ello deberás registrarte en el sistema bien de forma presencial, en
> alguna de las oficinas de registro adheridas al sistema, o bien por Internet,
> utilizando un certificado electrónico reconocido o, si no dispones de él, por
> carta de invitación o a través del registro mediante videoidentificación
> desde la app Cl@ve.»

### 7.1 Qué vía sirve en un nodo automatizado

| Vía | ¿Reutilizable desde un nodo sin guardar credenciales? | Por qué |
|---|---|---|
| **Cl@ve Móvil** | **Sí, es la buena** | El nodo **no** tiene la credencial: abre el flujo, la app del usuario confirma (QR + biometría). No hay contraseña que almacenar. La página oficial describe el ciclo QR → confirmar en la app → segundo factor (`ÍNDICE`, `https://clave.gob.es/clave-movil/uso-clave-movil`). |
| **Cl@ve Permanente** | **Sí, pero guardando un secreto** | Es usuario+contraseña: si el nodo la usa, es un **secreto más** de la bóveda (cifrado, 0600, con traza). No es "sin credenciales". |
| **Cl@ve PIN** | **Parcialmente** | Contraseña de validez muy limitada: sirve para autenticación esporádica, **no** como credencial reutilizable de un nodo. Además, muchos servicios de datos de salud exigen **registro avanzado**, no sólo un PIN. |

> **Cl@ve Móvil no es reutilizable por el nodo cuando el trámite requiere
> *firma***: la firma en la nube con Cl@ve Permanente es otro flujo y tiene sus
> propios requisitos (`NO VERIFICADO`: no he abierto la ficha de "firma en la
> nube" de `clave.gob.es`).

---

## 8. Certificados: FNMT, DNIe, PKCS#12 y mTLS

### 8.1 "Certificado de persona física" (FNMT / CERES)

De `https://www.sede.fnmt.gob.es/certificados/persona-fisica` (`FETCH`, cita
literal):

> «Existen cuatro formas distintas para obtener tu Certificado electrónico de
> Ciudadano como archivo descargable en tu ordenador o dispositivo móvil:
> - Con vídeo identificación.
> - Con acreditación presencial en una oficina.
> - Utilizando tu DNIe.
> - Utilizando tu Dispositivo Móvil.»

Y de la ayuda de la AEAT (`ÍNDICE`, cita literal):

> «Si ya dispones de certificado de persona física de la FNMT y solicitas otro
> certificado por la app móvil se revocará automáticamente el anterior, ya que
> solo se puede disponer de un único certificado de persona física por
> ciudadano, el mismo puede ser instalado tanto en un PC como en un dispositivo
> móvil utilizando la copia de seguridad.»

⇒ **Punto operativo importante**: pedir otro certificado **revoca** el anterior.
Un nodo que "re-solicite" el certificado del usuario le rompe el certificado
del móvil. La renovación la hace **el usuario**, en su navegador, y el nodo
sólo recibe el `.p12` resultante.

### 8.2 Formato, contraseña y copia de seguridad (PKCS#12 / PFX)

De la ayuda de la AEAT (`ÍNDICE`, citas literales):

> «El icono de una copia válida será un sobre abierto con un certificado
> asomando y una llave, y la extensión **.pfx o .p12**.»

> «[…] se le solicitará que introduzca (y confirmes) la contraseña de respaldo
> para hacer uso de esta copia.»

> «Es muy importante marcar también la casilla "Marcar la clave privada como
> exportable" y "Habilitar protección segura de clave privada…". […]
> Establece una contraseña y haz clic en "Finalizar".»

Y de la copia de seguridad en Safari (`ÍNDICE`, cita literal):

> «En "Formato de archivos" debe mostrarse la opción "Intercambio de
> información personal (.p12)". […] Introduce y confirma una contraseña para
> proteger la copia. […] El fichero obtenido tendrá la extensión .p12.»

Y de la descarga desde la FNMT (`ÍNDICE`, cita literal):

> «El navegador abrirá de nuevo la aplicación de generación de claves,
> introduce la contraseña que indicaste en el momento de la solicitud y si la
> contraseña es correcta se ofrecerá la opción de hacer copia de seguridad del
> certificado (Recomendado).»

⇒ **Modelo de despliegue**: `.p12` (PKCS#12) **con contraseña** es el formato
que viaja entre nodos; el harness lo guarda **cifrado con AEGIS-256 y clave
derivada con Argon2id** (ver `rust-core/src/cert_vault.rs`), nunca en claro.

### 8.3 DNIe

De la ayuda de la FNMT (`FETCH`, cita literal):

> «Para el uso del DNIe es necesario instalar el software criptográfico DNIe e
> instalar también los certificados raíces del DNIe […]»

> «Última versión de cualquiera de los siguientes navegadores: Mozilla Firefox,
> Google Chrome, Microsoft EDGE, Opera, Safari»

Y del RD 255/2025 (BOE, `ÍNDICE`, cita literal):

> «El plazo de validez de los certificados electrónicos contenidos en el
> Documento Nacional de Identidad será de **dos años**.»

⇒ El DNIe es un **certificado de tarjeta (hardware)**: la clave privada **no
sale del chip**. Es el ejemplo canónico de la clave **no exportable** del
harness: en el nodo sólo se guarda la referencia (alias del almacén), nunca el
material.

### 8.4 El "certificado de persona física de la AGE"

`NO VERIFICADO`. **No he encontrado ningún producto oficial con ese nombre
exacto.** Lo que sí existe y he verificado:

* El «Certificado electrónico de Ciudadano» de la FNMT (antes "certificado de
  persona física"), que es el certificado de persona física del mercado
  español (`FETCH`, `sede.fnmt.gob.es`).
* La **Política de Firma Electrónica y de Certificados de la Administración
  General del Estado** (`ÍNDICE`, `https://sede.administracion.gob.es/la-sede/politica-firma-electronica-certificados-age`)
  y sus **Perfiles de certificados electrónicos** (`ÍNDICE`,
  `administracionelectronica.gob.es/ctt/resources/.../Perfiles-de-certificados-electronicos-2-0.pdf`),
  que definen los perfiles de **persona física**, persona jurídica y empleado
  público dentro de la AGE: «Longitud mínima de clave pública del certificado
  de usuario: 2048 bits».
* El RD 255/2025 (`ÍNDICE`, BOE) que modifica el art. 27 del RD 203/2021: los
  «sistemas basados en certificados cualificados de firma electrónica […] para
  la identificación electrónica de persona física […] deberán contener como
  atributos, al menos, su nombre y apellidos y su número de Documento Nacional
  de Identidad, Número de Identificación de Extranjero, Número de Identidad
> Consular Central o Número de Identidad Fiscal».

⇒ **Conclusión honesta**: el "certificado de persona física de la AGE" que pide
el usuario **no es un producto distinto** del certificado de persona física
(que lo emite la FNMT/CERES y que la AGE admite); lo que la AGE tiene es la
**política de perfiles** que lo hace interoperable. Si el usuario se refiere al
certificado "de persona física" emitido por la **Seguridad Social** (que es
autoridad certificadora admitida por su propia sede, `ÍNDICE`), es otro
emisor, pero **NO VERIFICADO** su procedimiento exacto de solicitud para
ciudadanos.

---

## 9. Tabla resumen de mTLS y del papel de cada credencial

| Credencial | Dónde vive la clave privada | ¿Autenticación TLS de cliente (mTLS)? | ¿Reutilizable desde un nodo? | Notas |
|---|---|---|---|---|
| DNIe (chip) | **En el chip**, nunca sale | **Sí** | **No** (no se puede extraer) | Referencia en el harness = alias del almacén |
| Certificado FNMT de persona física (`.p12`) | En el fichero `.p12`, **cifrado con contraseña** | **Sí** | **Sí**, si el `.p12` viaja cifrado y se abre en memoria | Pedir otro **revoca** el anterior |
| Cl@ve Móvil | En el móvil del usuario (app) | No | **Sí** (flujo QR/push, sin credencial en el nodo) | La opción recomendada para automatizar |
| Cl@ve Permanente | Contraseña en el nodo + 2º factor | No | **Sí**, guardando la contraseña como secreto | Firma en la nube disponible |
| Cl@ve PIN | Contraseña de vida corta | No | **Parcial** | Para accesos esporádicos |
| Usuario/password de "Tu Seguridad Social" | Contraseña del usuario | No | Sí, como secreto | Requiere registro Cl@ve presencial previo |

---

## 10. Lo que este documento **no** permite decidir

1. **NO VERIFICADO**: qué trámites concretos de la Junta, la SJE o la Seguridad
   Social exigen **DNIe físico** (por ejemplo, los que piden "acreditación con
   documento original"). Eso hay que sacarlo del catálogo de procedimientos,
   trámite a trámite.
2. **NO VERIFICADO**: el detalle del **ciclo de vida** de una Cl@ve Permanente
   (caducidad, revocación, migración a Cl@ve Móvil) y si la firma en la nube
   cubre o no un trámite determinado.
3. **NO VERIFICADO**: requisitos técnicos de **conectividad** (whitelists de IP,
  proxy inverso) de las sedes: no he abierto las páginas de "Requisitos
   generales" ni de "certificados admitidos" en PDF.
4. **NO VERIFICADO**: si algún servicio concreto de SALUD Andalucía acepta
   **Cl@ve Móvil** (la app puede usar Cl@ve, pero la web de ClicSalud+ exige
   "certificado digital, DNie o Cl@ve" sin detallar la vía).
5. **NO VERIFICADO**: el detalle de la relación de responsabilidad (encargado
   vs autónomo) y la base jurídica concreta de cada SAD del SAS: hay que leer
   el aviso de privacidad de la app y el RGPD del SAS.

---

## 11. Fuentes (índice)

### Abiertas directamente (`FETCH`)
* `https://sedejudicial.justicia.es/firma-y-certificados-electronicos-admitidos`
* `https://www.sede.fnmt.gob.es/certificados/persona-fisica`
* `https://www.sede.fnmt.gob.es/certificados/persona-fisica/obtener-certificado-con-dnie/configuracion-previa`
* `https://sede.agenciatributaria.gob.es/Sede/ayuda/firma-digital-sistema-clave-pin-tecnica/certificados-electronicos.html`
* `https://sede.seg-social.gob.es/wps/portal/sede/sede/Inicio/RequisitosTecnicos/requisitos%20por%20tipo%20de%20acceso/`

### Contenido oficial recuperado vía índice (fetch directo agotó el tiempo de espera) (`ÍNDICE`)
* `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/ciudadania/app-salud-andalucia`
* `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/ciudadania/app-salud-andalucia/ayuda-para-la-obtencion-de-clve-y-su-uso-en-dispositivos-moviles`
* `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/ciudadania/clicsalud/salud`
* `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/ciudadania/farmacia-y-prestaciones/informacion-la-ciudadania-sobre-el-uso-de-medicamentos/receta-electronica-preguntas-frecuentes`
* `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/ayudadigital/aplicaciones/asistenciales/rxxi-prescripciones`
* `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/ayudadigital/aplicaciones/asistenciales/rxxi-dispensacion`
* `https://www.sspa.juntadeandalucia.es/servicioandaluzdesalud/sites/default/files/sincfiles/wsas-media-mediafile_sasdocumento/2024/manual_receta_electronica._receta_xxi._diciembre_2024._11_12_2024.pdf`
* `https://www.juntadeandalucia.es/servicios/sede.html`
* `https://www.juntadeandalucia.es/servicios/sede/sobre-sede/requisitos-acceso.html`
* `https://www.juntadeandalucia.es/servicios/sede/sobre-sede/ayuda/paginas/preguntas-frecuentes.html`
* `https://www.juntadeandalucia.es/servicios/sede/sobre-sede/sistemas-identificacion.html`
* `https://www.juntadeandalucia.es/servicios/tramites/servicios-digitales/notificaciones-electronicas.html`
* `https://www.juntadeandalucia.es/boja/2019/250/1` (Decreto 622/2019, anexos II y III)
* `https://clave.gob.es/clave/que-es`
* `https://clave.gob.es/clave-movil/que-es`
* `https://clave.gob.es/clave-movil/uso-clave-movil`
* `https://administracion.gob.es/tramites-electronicos/servicioselectronicosfrecuentes/necesito-registrarme-en-clave`
* `https://sede.seg-social.gob.es/wps/portal/sede/sede/Inicio/PreguntasFrecuentes`
* `https://sede.seg-social.gob.es/wps/portal/sede/sede/Inicio/CarpetaCiudadana`
* `https://sede.seg-social.gob.es/wps/portal/sede/sede/Inicio/informacionUtil/certificados+digitales/certificados+admitidos+en+los+tramites+de+la+seguridad+social`
* `https://sede-tu.seg-social.gob.es/` (Tu Seguridad Social)
* `https://www.seg-social.es/wps/portal/wss/internet/HerramientasWeb/4c5c5105-04d1-4dfd-8af4-1e2d5c0c1ae3/10432/51674`
* `https://www.administraciondejusticia.gob.es/es/-/soluciones-sede-judicial-electronica`
* `https://sedejudicial.justicia.es/preguntas-frecuentes`
* `https://sede.administracion.gob.es/la-sede/politica-firma-electronica-certificados-age`
* `https://administracionelectronica.gob.es/ctt/resources/Soluciones/239/Descargas/Perfiles-de-certificados-electronicos-2-0.pdf`
* `https://www.boe.es/eli/es/rd/2025/04/01/255/dof/spa/pdf` (Real Decreto 255/2025)
* `https://sede.agenciatributaria.gob.es/Sede/ayuda/firma-digital-sistema-clave-pin-tecnica/certificados-electronicos/informacion-pasos-obtencion-certificado-electronico/obtencion-certificado-electronico-mediante-dispositivo-movil.html`
