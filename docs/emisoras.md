# Emisoras de radio en directo

Diseño común de escritorio y móvil (KAN-160, epic KAN-159). Escritorio lo implementa primero
(KAN-161, KAN-162) y el móvil lo porta igual (KAN-163, KAN-164). Si algo de aquí cambia al
programarlo, se cambia aquí primero.

## Qué es

Escuchar emisoras de radio en directo dentro de Melomango: las de FM de siempre y las que solo
emiten por internet, con las de tu zona primero.

- Se escuchan **por internet**: la misma programación que la emisora da por FM, por su señal en
  streaming. Ni el Pixel 6a ni los ordenadores tienen sintonizador de FM.
- En la app se llama **«Emisoras»**, para no confundirlo con la **«Radio automática»** (canciones
  parecidas cuando se acaba la cola).
- Fuera de alcance por ahora: grabar, programar, podcasts y emisoras HLS (ver más abajo).

## De dónde salen: Radio Browser

[Radio Browser](https://www.radio-browser.info/) es un directorio libre de unas 50.000 emisoras,
sin clave ni cuenta («You may use it in free and non free software»; el servicio va «without
guarantee to work»). No publica una licencia de los datos: revisarlo en su repositorio antes de
publicar una versión con emisoras.

Datos de España (21/11/2026, `hidebroken=true`):

| | Emisoras | |
|---|---:|---|
| Total | 1218 | |
| MP3 | 974 | AAC 142, AAC+ 55, OGG 18, desconocido 22 |
| HLS | 75 | 6 % |
| Solo `http://` | 277 | 23 % |
| Con coordenadas | 367 | 30 % |
| Con región (`state`) | 652 | texto libre: «Madrid» y «Comunidad de Madrid», «Catalunya», «Catalonia» y «Barcelona» |
| Con `iso_3166_2` | 7 | casi vacío |

Las más escuchadas (Cadena 100, esRadio, LOS40 Classic, Cadena Dial, RNE…) son nacionales y
**no tienen coordenadas ni región**.

### Cómo se le pide

- **Servidor:** la lista de `https://all.api.radio-browser.info/json/servers` (o el DNS de
  `all.api.radio-browser.info`), en orden aleatorio; se usa el primero y, si una petición falla, el
  siguiente. Nunca un servidor fijo en el código (piden no hacerlo). Se recuerda la lista un día.
- **Cabecera:** `User-Agent: Melomango/<versión>` (escritorio) y `Melomango-Android/<versión>`
  (móvil). Siempre HTTPS. El cliente HTTP compartido, con sus tiempos máximos.
- **Siempre** `hidebroken=true` y sin HLS (`hls=0` se filtra en la app: la API no tiene parámetro).
- **Al empezar a escuchar** una emisora: `GET /json/url/{stationuuid}` (su contador de popularidad;
  cuenta una vez al día por IP y emisora). Sin esperar la respuesta ni avisar si falla.
- Páginas de 50 (`limit`, `offset`), ordenadas por `clickcount` descendente salvo que se diga otra cosa.

| Para | Petición |
|---|---|
| Cerca de ti | `/json/stations/search?geo_lat=…&geo_long=…&geo_distance=50000&hidebroken=true&limit=100` y se ordena **en la app** por `geo_distance` (metros) |
| Lo más escuchado del país | `/json/stations/search?countrycode=ES&order=clickcount&reverse=true&hidebroken=true&limit=50` |
| Buscar | `/json/stations/search?name=…&countrycode=…&order=clickcount&reverse=true&hidebroken=true&limit=100` con la palabra más larga no genérica, en tu país y si no en todo el mundo (sin `countrycode`), y en la app se quedan las que tienen **todas** las palabras (ver abajo); `tag=…` para géneros |
| Países (selector) | `/json/countries` (`name`, `iso_3166_1`, `stationcount`) |
| Una favorita al ponerla | `/json/stations/byuuid/{uuid}` (por si cambió su URL) |

## Modelo

Una emisora **no es una `Track`**: no tiene duración, no se descarga, no se empareja con YouTube,
no va a la caché automática y no cuenta como escucha en el historial ni en Tu resumen.

```text
Station {
  uuid:     stationuuid            // la clave
  name:     name, sin espacios al principio ni al final
  url:      url_resolved           // el stream de verdad
  homepage, favicon                // opcionales (favicon vacío → icono de radio)
  country:  countrycode            // "ES"
  state:    state                  // solo para mostrar («Catalunya»), nunca para filtrar
  tags:     tags separados por comas, en minúscula
  codec, bitrate                   // «AAC 128», «MP3 192»; bitrate 0 → solo el codec
  lat, long                        // opcionales
  distanceM                        // solo en «Cerca de ti» (geo_distance)
}
```

**Buscar:** la API busca el texto **seguido** dentro del nombre, sin distinguir mayúsculas ni
tildes: «Radio Girona» encuentra «Taronja ràdio Girona», pero «SER Miranda» no encuentra
«Cadena SER - Miranda de Ebro». Así que:

1. Se le pide la **palabra más larga que no sea genérica** («Miranda»). Las genéricas las llevan
   muchísimas emisoras en el nombre y no sirven para buscar: con «BBC radio», pedir «radio» devolvía
   cientos de emisoras sin «BBC» y ninguna de la BBC. Son: radio, radios, fm, am, online, web, live,
   stream, la, el, los, las, de, del, the, of, and, y, en, station, emisora. Si todas las palabras
   son genéricas («Radio FM»), la más larga.
2. La app se queda con las que tienen **todas** las palabras en el nombre, sin tildes ni mayúsculas
   (con `fold()` de `text.rs`; en el móvil, `words()`).
3. Primero **en tu país**; si ahí no hay ninguna, **en todo el mundo** («BBC radio» desde España
   encuentra BBC Radio 1 Dance).
4. Si tampoco y se buscó **una sola palabra**, se prueba como **género** (`tag`, en tu país):
   «jazz», «noticias». Con varias palabras no, porque saldrían emisoras que no tienen que ver.

El móvil lo hace así (`RadioBrowser.search` y `Stations.searchWord`, con sus tests).

Se descartan al leer: las HLS (`hls == 1`), las que no tienen `url_resolved` y las repetidas (misma
`url_resolved`, 6 en España; se queda la de más `clickcount`).

## Ubicación

De más a menos precisa, y siempre con la opción de elegir el país a mano:

1. **Cerca de ti**, con una **ubicación aproximada**, solo si el usuario la da:
   - Móvil: permiso `ACCESS_COARSE_LOCATION` (nunca el preciso), pedido **al pulsar «Cerca de ti»**
     con su explicación, y la posición de `LocationManager` (red); sin Google Play Services.
   - Escritorio: la ubicación de Windows (`Windows.Devices.Geolocation`), solo si se activa en
     Ajustes y Windows la permite.
   - Se redondea a **2 decimales** (~1 km) antes de usarla y se guarda redondeada, con la fecha,
     para no pedirla cada vez (se vuelve a pedir si tiene más de un día).
2. **Tu país**, sin pedir nada:
   - Móvil: el país de la red móvil (`TelephonyManager.networkCountryIso`), si no el de la SIM y si
     no el del idioma del móvil.
   - Escritorio: la región de Windows (`GetUserDefaultGeoName`).
3. **Elegido a mano** en un selector de países (con cuántas emisoras tiene), que se recuerda y
   tiene prioridad sobre el 2.

No hay selector de región: los datos de región están demasiado desordenados (ver la tabla). Una
emisora local se encuentra buscando («SER Miranda», «Ràdio Girona»).

**Privacidad:** solo se manda a Radio Browser el país o las coordenadas redondeadas, y nada más a
nadie. Sin ubicación, la pantalla funciona igual con el país.

## La pantalla «Emisoras»

En la barra lateral (escritorio) y en Biblioteca (móvil):

- **Cerca de ti**: las emisoras a menos de 50 km, de la más cercana a la más lejana, con «a 2 km».
  Debajo, **«Lo más escuchado en España»** (el país del paso 2 o 3): ahí están las nacionales, que
  no tienen coordenadas. Si no hay ubicación, solo esto, con «Activar ubicación» (móvil) o
  «Elegir país».
- **Buscar emisoras** por nombre o género.
- **Tus emisoras** (favoritas), con la más reciente primero.
- **Escuchadas hace poco**: las 10 últimas (en los ajustes, no en la base de datos).
- Cada emisora: logo (o el icono de radio), nombre, ciudad o región si la hay, distancia y calidad
  («AAC 128»), y el corazón para guardarla.

## Reproducir en directo

- **El reproductor en modo directo**: «En directo» en vez de la barra de progreso y la duración, el
  nombre de la emisora y **lo que suena** si la emisora lo manda (metadatos ICY, `StreamTitle`).
  Pausa, reanudar (vuelve al directo, no a donde se pausó) y volumen.
- **Siguiente y anterior** van a la emisora siguiente o anterior de la lista desde la que se puso.
  La cola, la radio automática, la letra, «No es esta canción», «Descargar» y aleatorio/repetir no
  aplican y no se muestran.
- **Escritorio:** la CSP solo deja `media-src 'self' stream: http://stream.localhost` (KAN-16), así
  que el audio pasa por el protocolo `stream:` de Rust, que hace de proxy del directo, pide
  `Icy-MetaData: 1`, quita los metadatos del audio y los manda a la interfaz como evento. Así
  tampoco hay que abrir la CSP, y funcionan las emisoras `http://`.
- **Móvil:** Media3 con la URL directa, **sin** `CacheDataSource` (un directo no se guarda) y con
  los metadatos ICY de Media3 (`IcyInfo`) en la notificación, la pantalla de bloqueo y el
  Bluetooth del coche. Para las 277 emisoras `http://` hay que permitir el tráfico sin cifrar en
  `network_security_config` (Android no deja limitarlo al audio). Las peticiones propias de la app
  siguen siendo todas HTTPS.
- **HLS:** fuera de la primera versión en las dos apps (6 % de las emisoras; WebView2 no lo
  reproduce en `<audio>` sin hls.js), para que las dos muestren las mismas.
- **Media Session y Discord:** nombre de la emisora, lo que suena y el logo.

## Favoritas

Una tabla nueva en la biblioteca. Se guarda la emisora entera para poder mostrarla sin red.

```sql
CREATE TABLE IF NOT EXISTS stations (
    uuid       TEXT PRIMARY KEY,      -- stationuuid
    name       TEXT NOT NULL,
    url        TEXT NOT NULL,         -- url_resolved al guardarla
    homepage   TEXT,
    favicon    TEXT,
    country    TEXT,
    state      TEXT,
    tags       TEXT,
    codec      TEXT,
    bitrate    INTEGER NOT NULL DEFAULT 0,
    lat        REAL,
    long       REAL,
    liked_at   INTEGER NOT NULL
);
```

- **Escritorio:** en `ADDITIVE` (`CREATE TABLE IF NOT EXISTS`), sin subir `SCHEMA_VERSION`, para
  que una versión anterior siga abriendo la biblioteca.
- **Móvil:** entidad de Room y base de datos de la versión 1 a la 2 con una migración (y su esquema
  exportado en `schemas/`). Entra en la copia de seguridad de Android con la biblioteca.
- Al poner una favorita se pide `byuuid` para tener su URL al día; si no responde, la guardada.

## Textos

| Dónde | Texto |
|---|---|
| Barra lateral / Biblioteca | Emisoras |
| Pantalla | Cerca de ti · Lo más escuchado en {país} · Tus emisoras · Escuchadas hace poco |
| Buscador | Buscar emisoras |
| Sin ubicación (móvil) | Activa la ubicación aproximada para ver las emisoras de tu zona. Solo se usa para eso. |
| Botones | Activar ubicación · Elegir país · Guardar en tus emisoras · Quitar de tus emisoras |
| Reproductor | En directo |
| Cada emisora | a {n} km · AAC 128 |
| Cargando | Sintonizando… |
| Sin resultados | No hay emisoras con ese nombre. Prueba con otro o con un género («rock», «noticias»). |
| Error de la emisora | Esta emisora no responde ahora mismo. Prueba con otra. |
| Error del directorio | No se pudo cargar la lista de emisoras. Comprueba tu conexión y vuelve a intentarlo. |
| Avisos | «{emisora}» guardada en tus emisoras · «{emisora}» quitada de tus emisoras |

## Hecho cuando

En las dos apps se pueden ver las emisoras de cerca (con ubicación) o del país (sin ella),
buscarlas, guardarlas y escucharlas en directo con lo que suena, sin tocar la cola, el historial
ni Tu resumen, y con los mismos textos.
