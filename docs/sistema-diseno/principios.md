# Principios de diseño, identidad y tono

KAN-86, de la épica KAN-84. Los tokens y los componentes dicen cómo se ve cada cosa; esto dice qué
hacer cuando no hay regla. Vale para Melomango escritorio y Melomango móvil. Lo que hay hoy está
en la [auditoría](auditoria.md), y sus capturas sirven de ejemplo.

## Principios

### 1. La música primero

Las carátulas, la letra y lo que suena mandan; la interfaz se aparta. El fondo es crema o hueso
tostado, los controles van atenuados hasta que se necesitan y no hay pancartas ni adornos que
compitan con la portada.

- **Bien:** la pantalla Sonando del móvil, con la carátula a todo el ancho. El panel de letra de
  escritorio, con la línea actual resaltada y el resto atenuado. Los iconos de la fila en
  `--muted`, que pasan a mango solo al pasar el ratón.
- **Mal:** fondos de color detrás de las listas o carátulas pequeñas en la pantalla principal del
  reproductor. Las franjas negras que hoy trae la miniatura de YouTube (error 2 de la auditoría):
  la carátula tiene que llenar su hueco.

### 2. Cálido, no chillón

El naranja mango es el **acento**, no el relleno: el botón principal de cada pantalla, lo que
suena, la pestaña o la opción elegida y el corazón de Me gusta. Todo lo demás va en crema, hueso y
marrón.

- **Bien:** un solo botón mango por cabecera («Reproducir») y los demás secundarios. La fila que
  suena, con el título en `--accent-text` sobre `--surface-hover`.
- **Mal:** superficies grandes en mango, texto largo en naranja, o dos botones mango juntos. Y el
  texto pequeño en `--accent` sobre crema, que no se lee (1,9:1): para texto está `--accent-text`.

### 3. Una sola app en dos pantallas

Melomango se ve y habla igual en el ordenador y en el móvil: mismos colores, mismas palabras,
mismos iconos y mismas portadas. Lo que cambia es **cómo se usa**: ratón y teclado en un sitio,
toques y gestos en el otro (ver [Plataforma](#plataforma)).

- **Bien:** «Me gusta», «Añadir a la cola» y «No es esta canción…» dicen lo mismo en las dos. La
  pantalla Emisoras se diseñó una vez (`docs/emisoras.md`) y se hizo dos veces.
- **Mal:** la portada de Me gusta en naranja plano en el móvil y con degradado en escritorio, o
  «Repetir lista» en una app y «Repetir la lista» en la otra.

### 4. Siempre se sabe qué pasa

Cada espera, cada vacío y cada fallo se explica con una frase corta: qué está pasando y, si
depende del usuario, qué puede hacer. Nunca una pantalla en blanco ni un error técnico solo.

- **Bien:** «Aún no has descargado nada. Usa el botón Descargar de una playlist o de una canción».
  «Esta emisora no responde ahora mismo. Prueba con otra». «Sintonizando…».
- **Mal:** «No se pudo guardar: TypeError: …» (el texto técnico va en «Detalles», no en el
  mensaje). Una lista vacía sin decir por qué.

### 5. Lo tuyo se queda contigo

Melomango no tiene cuentas y guarda todo en el equipo o en el móvil. Cuando algo sale de ahí, se
dice para qué y se pide antes: la ubicación aproximada de las emisoras, la conexión con Spotify,
Discord.

- **Bien:** «Solo se guarda en tu equipo» en el Historial. «Activa la ubicación aproximada para ver
  las emisoras de tu zona. Solo se usa para eso».
- **Mal:** pedir un permiso al abrir la app, sin que el usuario haya pulsado nada que lo necesite.

## Identidad

**El nombre** es Melomango: una palabra, con M mayúscula. El lema es «Sin pagar ni un mango».

**El logo** está en [`app-icon.svg`](../../app-icon.svg): el mango con la nota musical. De él salen
el icono de la app (escritorio con `tauri icon`; el móvil, en `res/mipmap-anydpi` y
`res/drawable/ic_launcher_*`) y la marca de la barra superior de escritorio. Allí va a 34 px, con
una sombra muy suave y junto al nombre escrito con el degradado mango.

- **Sí:** sobre crema o sobre hueso tostado, con aire alrededor de al menos la mitad de su ancho.
- **No:** cambiarle los colores, girarlo, estirarlo, ponerle contornos, ponerlo sobre el degradado
  mango (se pierde) o usarlo como icono dentro de la interfaz.

**El degradado mango** (`#ffd25e → #ffa21f → #ff7a1a`, a 135°) es la firma de Melomango. Hay uno
solo.

- **Dónde va:** en el nombre de la barra superior, en el botón de reproducir del reproductor, en la
  portada de Me gusta y en las cabeceras de las secciones de Melomango (Historial, Tu resumen,
  Descargas, Emisoras).
- **Dónde no:** en fondos grandes, ni detrás de texto pequeño. El texto blanco encima no llega al
  contraste mínimo (lo resuelve KAN-88).
- **Hoy hay dos degradados en escritorio:** las cabeceras de Historial, Tu resumen, Descargas y
  Emisoras, y las de artista y álbum sin foto, usan otro de dos paradas (`--accent → --accent-deep`).
  Pasan a `--mango-gradient` con KAN-97.
- **El móvil** todavía no tiene el degradado; KAN-87 lo pasa a token.

**Portadas generadas,** cuando no hay imagen. El icono dice qué es y es decorativo, porque el
nombre va siempre al lado. Hay dos clases y se ven igual en las dos apps:

- **Lo que es de Melomango** (Me gusta y las cabeceras de sus secciones): el degradado mango con el
  icono en blanco.
- **Lo que aún no tiene imagen** (playlist vacía, canción sin carátula, emisora sin logo, artista
  sin foto): el fondo suave `--cover-soft` con el icono en `--accent-text`. No compite con las
  carátulas de verdad que tiene al lado.

| Qué | Clase | Icono | Hoy en escritorio | Hoy en el móvil |
|---|---|---|---|---|
| Me gusta | Melomango | corazón | así | **mango plano y corazón oscuro** |
| Historial, Tu resumen, Descargas, Emisoras | Melomango | reloj, gráfica, descarga, radio | **el otro degradado** | iconos sobre `surface-soft` en Biblioteca |
| Playlist | — | mosaico 2×2 con 4 carátulas o más; si no, la primera | así | así |
| Playlist vacía | sin imagen | nota | así | **`surfaceContainerHigh` con nota** |
| Canción sin carátula | sin imagen | nota | **degradado sin icono** | **`surfaceContainerHigh` con nota** |
| Emisora sin logo | sin imagen | radio | **degradado con radio blanca** | **`surfaceContainerHigh` con radio** |
| Artista o álbum sin foto | sin imagen | persona, disco | **el otro degradado** en las cabeceras | **`surfaceContainerHigh` con nota** (también el artista) |

**El humor** tiene su sitio, pero con medida: «Pelando mangos…» mientras carga, «Ni un mango.
Prueba otra búsqueda.» cuando no hay resultados y «Elige algo, melómano» con el reproductor vacío.
Va solo en esperas y vacíos, nunca en errores, confirmaciones ni nada que tenga que ver con
borrar, pagar o la privacidad.

## Tono de los textos

- **Español de España, de tú.** «Elige», «Activa», «Pon algo de música»; nunca «elija» ni «usted».
- **Frases cortas y sin jerga.** «Sin conexión», no «Error de red». «Esta emisora no responde», no
  «Timeout del stream». Los nombres de servicios se escriben como ellos (YouTube, Deezer, LRCLIB,
  Radio Browser).
- **Mayúscula solo al principio** y en los nombres propios: «Tu resumen», «Añadir a la cola». Los
  antetítulos de las cabeceras se escriben «Colección», «Artista» y «Álbum»; si se ven en
  mayúsculas, es cosa del estilo, no del texto.
- **Comillas españolas « »** para citar nombres: «La Perla» añadida a la cola.
- **Puntos suspensivos (…, un solo carácter)** en lo que está en marcha («Cargando…»,
  «Descargando 3…») y en los botones que abren algo que pide más antes de hacer nada («Nueva
  playlist…», «No es esta canción…»).
- **Números** con el formato español: 1.072.023, 0,5 s, 2 GB. Las cantidades concuerdan: «1
  canción», «3 canciones».

**Botones y opciones:** un verbo en infinitivo que dice lo que pasa al pulsar: «Descargar», no
«Descarga»; «Guardar en tus emisoras», «Borrar historial». Hay tres excepciones:
- los destinos de la navegación son sitios y van con nombre: «Inicio», «Descargas», «Emisoras»;
- los interruptores describen lo que activan: «Normalizar el volumen», «Emisoras cerca de ti»;
- en los diálogos, el botón que confirma repite el verbo de la pregunta: «¿Borrar el historial?» →
  «Borrar» y «Cancelar», nunca «Aceptar» ni «Sí».

**Errores: qué ha pasado y qué hacer.**
- Bien: «No hay conexión con el servicio. Comprueba tu conexión a internet y vuelve a intentarlo».
  «YouTube pide confirmar que no eres un robot. Espera unos minutos y vuelve a intentarlo».
- El texto técnico va en «Detalles» (`ErrorNotice` y `friendlyError` en las dos apps), **nunca
  pegado detrás** del mensaje con dos puntos.
- Si reintentar puede ayudar, se ofrece «Reintentar».
- Sin culpar al usuario («No has…») y sin disculpas vacías («Ups», «Lo sentimos»).

**Estados vacíos:** qué es esto y cómo se llena. «Crea tu primera playlist con el botón +, o desde
el + de cualquier canción».

**Cargando:** «Cargando…» o, si se sabe, qué se carga: «Cargando el álbum…». Hay dos con
personalidad, «Pelando mangos…» (canciones) y «Sintonizando…» (emisoras).

**Avisos (toast o snackbar):** el hecho, con el nombre entre comillas españolas: «La Perla»
sonará a continuación; Historial borrado. Si se puede deshacer, el botón dice «Deshacer» (como ya
hace el móvil al quitar de la cola).

**Confirmaciones de borrar:** la pregunta, lo que se pierde y lo que no. «Se borrará todo lo que
has escuchado. Tus Me gusta y playlists no cambian».

**El sistema por su nombre:** «Como Windows» y «tu equipo» en escritorio; «Como el móvil» y «tu
móvil» en Android.

## Glosario

Las dos apps usan estas palabras y solo estas.

| Di | Para | No digas |
|---|---|---|
| **Me gusta** | la colección y la acción: «Añadir a Me gusta», «Quitar de Me gusta», «Tus Me gusta» | Favoritos, Mis canciones, Likes |
| **Playlist** | una lista de canciones del usuario o pública (Deezer, Spotify) | lista de reproducción, lista (salvo en «Repetir la lista»), playlists |
| **Biblioteca** | todo lo del usuario: Me gusta, playlists, descargas, emisoras, historial | Colección (solo es el antetítulo), Mi música |
| **Cola** | lo que va a sonar: «Añadir a la cola» | lista de espera |
| **A continuación** | justo después de la actual: «Reproducir a continuación» | siguiente, después |
| **Radio automática** | canciones parecidas al acabar la cola | autoplay, mix |
| **Iniciar radio** | empezar canciones parecidas a una canción o un artista | Radio (solo), crear radio |
| **Emisoras** | radio en directo, por internet. «Tus emisoras» son las guardadas | radios, estaciones, cadenas |
| **En directo** | lo que suena en una emisora | en vivo, live |
| **Canción** | cada pista | tema, pista, track |
| **Artista**, **Álbum**, **Sencillos y EP** | lo que dice Deezer | grupo, disco, singles |
| **Letra** | la letra de la canción que suena | letras, lyrics |
| **Descargar**, **Descargas**, **descargada** | guardar para escuchar sin conexión, el sitio donde está y su estado | bajar, offline |
| **Caché automática** | lo que se guarda solo | caché temporal, buffer |
| **Historial** | lo escuchado, por días | actividad, reproducciones |
| **Tu resumen** | las estadísticas del mes | estadísticas, Wrapped |
| **Inicio**, **Buscar**, **Ajustes** | las pantallas | Home, Explorar, Configuración, Preferencias |
| **Importar**, **Sincronizar** | traer una playlist de otro servicio y ponerla al día | exportar (al revés), refrescar |
| **Quitar** | sacar algo de un sitio sin destruirlo: de Me gusta, de la cola, de una playlist, una descarga de la lista | eliminar, borrar |
| **Borrar** | destruir datos o texto: descargas del disco, el historial, el texto de un campo | quitar, limpiar, vaciar (salvo «Vaciar» la caché o la cola) |
| **Eliminar** | solo playlists (la playlist deja de existir) | borrar playlist |
| **Aleatorio**, **Repetir la lista**, **Repetir esta canción** | los modos del reproductor | mezclar, bucle, Repetir lista |
| **No es esta canción…** | elegir otra versión de una canción importada | cambiar coincidencia, match |
| **Minirreproductor** | el reproductor pequeño | Mini reproductor, mini player |
| **Plugin**, **fuente** (escritorio) | lo que añade de dónde sale la música, y cada sitio de donde sale | extensión, complemento |
| **Reintentar**, **Detalles**, **Cerrar**, **Cancelar**, **Atrás** | las acciones de siempre | Volver a intentar (en el botón), Más info, Salir, OK |

## Plataforma

| Se comparte siempre | Cambia a propósito |
|---|---|
| Colores, degradado y contraste (tokens de KAN-87) | Navegación: barra lateral y Alt+← en escritorio; barra inferior y atrás del sistema en el móvil |
| Escala de tamaños y pesos de letra (la fuente puede ser la del sistema, lo decide KAN-89) | Densidad: filas con columnas y número en escritorio; filas compactas con menú ⋮ en el móvil. Las zonas de toque del móvil miden al menos 48 dp |
| Radios, espaciado y movimiento (tokens) | Hover, atajos de teclado y menús emergentes en escritorio; pulsación larga, gestos y hojas inferiores en el móvil |
| Iconos (un solo juego, KAN-91) | Diálogos en escritorio (Ajustes, Importar); pantallas completas en el móvil |
| Textos y glosario de esta página | Reproductor: barra fija con paneles y minirreproductor en otra ventana; mini sobre la barra inferior y pantalla Sonando |
| Portadas generadas y carátula vacía | Integraciones del sistema: bandeja y Discord en escritorio; notificación, pantalla de bloqueo, Android Auto y Bluetooth en el móvil |
| Qué información lleva cada componente y en qué orden | El nombre del sistema en los textos («Como Windows» / «Como el móvil») |
| Estados vacíos, de carga y de error | Opciones que solo tienen sentido en una: fundido y normalizar en escritorio (hasta KAN-41), plugins |

## Textos que hay que cambiar

La revisión de los textos de las dos apps con este glosario. Los de escritorio van con KAN-98 y los
del móvil con KAN-100. Los marcados **E+M** se cambian en las dos.

| # | App | Dónde | Ahora | Pasa a |
|---|---|---|---|---|
| 1 | E+M | Reproductor y cola (`PlayerBar.tsx`, `QueuePanel.tsx`; `PlayerViews.kt`, `PlaybackService.kt`) | «Repetir lista», «Repetir canción»; en escritorio también «Repetir la lista», «Repetir esta canción» | «Repetir la lista», «Repetir esta canción» en todas partes, también en la notificación del móvil |
| 2 | E | `QueuePanel.tsx`, `title` del interruptor | «Cuando se acaba la cola, sigue con canciones parecidas» | «Al acabar la cola, canciones parecidas» (como su nota y como el móvil) |
| 3 | E+M | Emisoras: subtítulo (`StationsView.tsx`; `StationsScreen.kt`) | «Radio por internet, con las de tu zona primero» / «En directo, con las de tu zona primero» | «Radio en directo, con las de tu zona primero» (como Biblioteca en el móvil) |
| 4 | E | `App.tsx`, `CatalogViews.tsx`, `downloads.tsx`, `library.tsx` | «No se pudo cambiar al mini reproductor: ${e}», «No se pudo guardar: ${e}», «No se pudo descargar: ${e}», «No se pudo quitar la descarga: ${e}», «No se pudo cargar la biblioteca: ${e}» | el mensaje de `friendlyError(e)` y el texto técnico en «Detalles»: «No se pudo descargar. Comprueba tu conexión y vuelve a intentarlo.» |
| 5 | M | `Updater.kt` | «No se pudo instalar la actualización: $it», «la descarga llegó incompleta ($done de … bytes)», «la descarga no coincide con la versión publicada (SHA-256 distinto)» | «No se pudo instalar la actualización. Vuelve a intentarlo.» y «La actualización no se descargó entera. Vuelve a intentarlo.», con el detalle en «Detalles» |
| 6 | M | `SpotifyExport.kt`, lista de problemas | «archivo: ${e.message}» (el mensaje de la excepción, a veces en inglés) | una frase por archivo: «Playlist1.json: no se pudo leer» |
| 7 | E | `PlayerBar.tsx`, `App.tsx`, README | «Mini reproductor», «mini reproductor» | «Minirreproductor» |
| 8 | E+M | Página de artista, botón (`CatalogViews.tsx`; `CatalogScreens.kt`) | «Radio» | «Iniciar radio» (como en el menú de la canción) |
| 9 | E | `HomeView.tsx`, error de tendencias | «No se pudieron cargar las tendencias. Ver por qué» | `ErrorNotice`, con «Reintentar» y «Detalles» |
| 10 | M | `SearchScreen.kt`, `SeeAll` | «Ver todas» en todas las secciones | concordancia: «Ver todas» (canciones, playlists), «Ver todos» (artistas, álbumes), como en escritorio |
| 11 | M | `TrackTools.kt`, botón del filtro | «Borrar filtro» | «Borrar el filtro» (como «Borrar la búsqueda») |
| 12 | E | `SettingsDialog.tsx`, nota de «Emisoras cerca de ti» | «…Solo se usa para eso y no sale de tu equipo salvo para pedir la lista.» | «Solo se usa para eso: Melomango manda tu zona, redondeada a 1 km, para pedir la lista.» (dice qué sale en vez de negarlo a medias) |
| 13 | E+M | Antetítulos de las cabeceras | «COLECCIÓN» (en el texto en mayúsculas) / «Colección» | el texto siempre «Colección», «Artista», «Álbum»; las mayúsculas, si se quieren, con el estilo (KAN-89) |

Y dos cambios de imagen que salen de la identidad, no de los textos: las portadas generadas según
la tabla de [Identidad](#identidad) (escritorio: canción sin carátula y emisora sin logo; móvil: Me
gusta y carátula vacía), y un único degradado.

**Revisados y sin cambios:** Me gusta, Cola, A continuación, Descargas, Historial, Tu resumen,
Ajustes, Importar, Sincronizar, Aleatorio, «No es esta canción…», los textos de las emisoras
(comunes desde `docs/emisoras.md`) y los mensajes de `errors.ts` / `Errors.kt`, que ya son iguales
en las dos apps. No hay textos de usted, ni «Aceptar», ni disculpas.
