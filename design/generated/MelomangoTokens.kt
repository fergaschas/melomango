// Generado por scripts/tokens.ts desde design/tokens/ (versión 1.0.0). No lo edites a mano:
// cambia los tokens y ejecuta `npm run tokens` en el repo melomango.
@file:Suppress("MagicNumber")

package com.fgascon.melomango.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/** Los colores de un tema de Melomango. Cada uno lleva su nombre en CSS para buscarlo en escritorio. */
class MelomangoColors(
    /** Fondo de la app. (`--color-bg`) */
    val bg: Color,
    /** Superficie: listas, tarjetas, barra del reproductor. (`--color-surface`) */
    val surface: Color,
    /** Superficie suave: fila al pasar el ratón, campos, paneles. (`--color-surface-soft`) */
    val surfaceSoft: Color,
    /** Lo elegido: la fila que suena, la entrada activa de la barra lateral. (`--color-surface-hover`) */
    val surfaceHover: Color,
    /** Diálogos, menús y hojas. (`--color-surface-raised`) */
    val surfaceRaised: Color,
    /** Bordes y separadores (decorativos). (`--color-border`) */
    val border: Color,
    /** Texto principal. (`--color-text`) */
    val text: Color,
    /** Texto secundario: artista, duración, notas. Pasa AA en todas las superficies. (`--color-text-muted`) */
    val textMuted: Color,
    /** Texto desactivado (sin requisito de contraste). (`--color-text-disabled`) */
    val textDisabled: Color,
    /** Relleno de acento: el botón principal, lo elegido. No para texto sobre claro. (`--color-accent`) */
    val accent: Color,
    /** Acento intenso: hover del acento, iconos de marca grandes. (`--color-accent-deep`) */
    val accentDeep: Color,
    /** Texto e iconos naranjas sobre superficies claras. (`--color-accent-text`) */
    val accentText: Color,
    /** Texto e iconos sobre el acento o el degradado (oscuro: el blanco no llega a 3:1). (`--color-accent-on`) */
    val accentOn: Color,
    /** Acento sobre una superficie invertida (el botón de un aviso). (`--color-accent-inverse`) */
    val accentInverse: Color,
    /** El degradado mango, a 135°: la firma de Melomango. (`--color-accent-gradient`) */
    val accentGradient: List<Pair<Float, Color>>,
    /** Anillo de foco del teclado (3:1 o más sobre todas las superficies). (`--color-focus`) */
    val focus: Color,
    /** Halo suave alrededor de un campo enfocado. (`--color-focus-halo`) */
    val focusHalo: Color,
    /** Fondo al pasar el ratón por algo que se puede pulsar. (`--color-state-hover`) */
    val stateHover: Color,
    /** Fondo mientras se pulsa o de lo que está activo. (`--color-state-pressed`) */
    val statePressed: Color,
    /** Texto e icono de éxito. (`--color-success`) */
    val success: Color,
    /** Fondo de un aviso de éxito. (`--color-success-bg`) */
    val successBg: Color,
    /** Texto e icono de aviso. (`--color-warning`) */
    val warning: Color,
    /** Fondo de un aviso. (`--color-warning-bg`) */
    val warningBg: Color,
    /** Texto e icono de error. (`--color-danger`) */
    val danger: Color,
    /** Fondo de un error. (`--color-danger-bg`) */
    val dangerBg: Color,
    /** Texto e icono informativo. (`--color-info`) */
    val info: Color,
    /** Fondo de un aviso informativo. (`--color-info-bg`) */
    val infoBg: Color,
    /** Líneas de la letra ya cantadas: el texto al 60 %. (`--color-lyric-past`) */
    val lyricPast: Color,
    /** La línea que suena (más oscura que el texto naranja: va sobre el panel mango). (`--color-lyric-current`) */
    val lyricCurrent: Color,
    /** Líneas que vienen: el texto al 50 % (3:1, texto grande). (`--color-lyric-future`) */
    val lyricFuture: Color,
    /** Velo detrás de un diálogo. (`--color-backdrop`) */
    val backdrop: Color,
    /** Brillo de la parte de arriba del contenido. (`--color-decor-glow-top`) */
    val decorGlowTop: Color,
    /** Mitad del brillo del contenido, antes del fondo. (`--color-decor-glow-mid`) */
    val decorGlowMid: Color,
    /** Fondo de los paneles de letra y cola. (`--color-decor-panel`) */
    val decorPanel: List<Pair<Float, Color>>,
    /** Portada de lo que aún no tiene imagen (playlist vacía, canción sin carátula…). (`--color-decor-cover-soft`) */
    val decorCoverSoft: List<Pair<Float, Color>>,
    /** Cristal sobre el brillo (botones de la cabecera). (`--color-decor-glass`) */
    val decorGlass: Color,
    /** Barra superior translúcida. (`--color-decor-topbar`) */
    val decorTopbar: Color,
    /** Etiqueta de plugin. (`--color-decor-plugin-badge`) */
    val decorPluginBadge: Color,
    /** Datos 1; jugador 1 del karaoke. (`--color-data-1`) */
    val data1: Color,
    /** Datos 2; jugador 2 del karaoke. (`--color-data-2`) */
    val data2: Color,
    /** (`--color-data-3`) */
    val data3: Color,
    /** (`--color-data-4`) */
    val data4: Color,
    /** (`--color-data-5`) */
    val data5: Color,
    /** (`--color-data-6`) */
    val data6: Color,
)

/** Los tokens de diseño, los mismos que `src/tokens.css` de escritorio. */
object MelomangoTokens {
    const val VERSION = "1.0.0"

    /** Tema claro: crema y mango. */
    val light = MelomangoColors(
        bg = Color(0xFFFFFAF2),
        surface = Color(0xFFFFFFFF),
        surfaceSoft = Color(0xFFFFF3DF),
        surfaceHover = Color(0xFFFFECCC),
        surfaceRaised = Color(0xFFFFFFFF),
        border = Color(0xFFF2E2C6),
        text = Color(0xFF2E1F0F),
        textMuted = Color(0xFF7C674E),
        textDisabled = Color(0xFFB8A68F),
        accent = Color(0xFFFFA21F),
        accentDeep = Color(0xFFF07800),
        accentText = Color(0xFFAD5000),
        accentOn = Color(0xFF2E1F0F),
        accentInverse = Color(0xFFFFA21F),
        accentGradient = listOf(0f to Color(0xFFFFD25E), 0.55f to Color(0xFFFFA21F), 1f to Color(0xFFFF7A1A)),
        focus = Color(0xFFAD5000),
        focusHalo = Color(0x2EFFA21F),
        stateHover = Color(0xFFFFF3DF),
        statePressed = Color(0xFFFFECCC),
        success = Color(0xFF2E7D32),
        successBg = Color(0xFFE8F5E9),
        warning = Color(0xFF8A5A00),
        warningBg = Color(0xFFFFF4D6),
        danger = Color(0xFFC62828),
        dangerBg = Color(0xFFFDECEA),
        info = Color(0xFF1F5F99),
        infoBg = Color(0xFFE7F1FB),
        lyricPast = Color(0x992E1F0F),
        lyricCurrent = Color(0xFFA34B00),
        lyricFuture = Color(0x802E1F0F),
        backdrop = Color(0x473C230A),
        decorGlowTop = Color(0xFFFFE6B3),
        decorGlowMid = Color(0xFFFFF3DF),
        decorPanel = listOf(0f to Color(0xFFFFF3DF), 1f to Color(0xFFFFE6B3)),
        decorCoverSoft = listOf(0f to Color(0xFFFFE6B3), 1f to Color(0xFFFFC46B)),
        decorGlass = Color(0x8CFFFFFF),
        decorTopbar = Color(0xD9FFFAF2),
        decorPluginBadge = Color(0xFFFFE6B3),
        data1 = Color(0xFFB06D00),
        data2 = Color(0xFF2A7FB8),
        data3 = Color(0xFF00805C),
        data4 = Color(0xFFA6508A),
        data5 = Color(0xFF8F1D1D),
        data6 = Color(0xFF24549C),
    )

    /** Tema oscuro: hueso tostado. */
    val dark = MelomangoColors(
        bg = Color(0xFF17110B),
        surface = Color(0xFF221910),
        surfaceSoft = Color(0xFF2C2014),
        surfaceHover = Color(0xFF362818),
        surfaceRaised = Color(0xFF221910),
        border = Color(0xFF3D2D1C),
        text = Color(0xFFF7EADB),
        textMuted = Color(0xFFB59D80),
        textDisabled = Color(0xFF6B5A47),
        accent = Color(0xFFFFA21F),
        accentDeep = Color(0xFFFF8F1F),
        accentText = Color(0xFFFFB85A),
        accentOn = Color(0xFF17110B),
        accentInverse = Color(0xFFAD5000),
        accentGradient = listOf(0f to Color(0xFFFFD25E), 0.55f to Color(0xFFFFA21F), 1f to Color(0xFFFF7A1A)),
        focus = Color(0xFFFFA21F),
        focusHalo = Color(0x2EFFA21F),
        stateHover = Color(0xFF2C2014),
        statePressed = Color(0xFF362818),
        success = Color(0xFF7FD08A),
        successBg = Color(0xFF1F3323),
        warning = Color(0xFFFFD27A),
        warningBg = Color(0xFF3A2A0C),
        danger = Color(0xFFFF8A80),
        dangerBg = Color(0xFF3D1F1C),
        info = Color(0xFF8CC4FF),
        infoBg = Color(0xFF15263A),
        lyricPast = Color(0x94F7EADB),
        lyricCurrent = Color(0xFFFFB85A),
        lyricFuture = Color(0x66F7EADB),
        backdrop = Color(0x8C000000),
        decorGlowTop = Color(0xFF3A240C),
        decorGlowMid = Color(0xFF24180D),
        decorPanel = listOf(0f to Color(0xFF24180D), 1f to Color(0xFF33200B)),
        decorCoverSoft = listOf(0f to Color(0xFF5A3A12), 1f to Color(0xFFA8641A)),
        decorGlass = Color(0x12FFFFFF),
        decorTopbar = Color(0xD917110B),
        decorPluginBadge = Color(0xFF4A3214),
        data1 = Color(0xFFE69F00),
        data2 = Color(0xFF56B4E9),
        data3 = Color(0xFF009E73),
        data4 = Color(0xFFCC79A7),
        data5 = Color(0xFFE5534B),
        data6 = Color(0xFF4C8FD6),
    )

    /** Alto de la barra del reproductor de escritorio. (`--size-player-height`) */
    val sizePlayerHeight = 92.dp
    /** Opacidad de un control desactivado. (`--opacity-disabled`) */
    val opacityDisabled = 0.4f

    /** Los roles de Material 3, según design/tokens/material3.json. */
    val lightColorScheme: ColorScheme = lightColorScheme(
        primary = light.accent,
        onPrimary = light.accentOn,
        primaryContainer = light.surfaceHover,
        onPrimaryContainer = light.accentText,
        inversePrimary = light.accentInverse,
        secondary = light.accentDeep,
        onSecondary = light.accentOn,
        secondaryContainer = light.border,
        onSecondaryContainer = light.text,
        tertiary = light.accentText,
        onTertiary = light.surface,
        tertiaryContainer = light.surfaceHover,
        onTertiaryContainer = light.accentText,
        background = light.bg,
        onBackground = light.text,
        surface = light.surface,
        onSurface = light.text,
        surfaceVariant = light.surfaceSoft,
        onSurfaceVariant = light.textMuted,
        surfaceTint = light.accent,
        inverseSurface = light.text,
        inverseOnSurface = light.bg,
        error = light.danger,
        onError = light.surface,
        errorContainer = light.dangerBg,
        onErrorContainer = light.danger,
        outline = light.border,
        outlineVariant = light.border,
        scrim = light.backdrop,
        surfaceBright = light.surface,
        surfaceDim = light.surfaceSoft,
        surfaceContainerLowest = light.surface,
        surfaceContainerLow = light.surfaceSoft,
        surfaceContainer = light.surfaceSoft,
        surfaceContainerHigh = light.surfaceHover,
        surfaceContainerHighest = light.border,
    )

    val darkColorScheme: ColorScheme = darkColorScheme(
        primary = dark.accent,
        onPrimary = dark.accentOn,
        primaryContainer = dark.surfaceHover,
        onPrimaryContainer = dark.accentText,
        inversePrimary = dark.accentInverse,
        secondary = dark.accentDeep,
        onSecondary = dark.accentOn,
        secondaryContainer = dark.border,
        onSecondaryContainer = dark.text,
        tertiary = dark.accentText,
        onTertiary = dark.surface,
        tertiaryContainer = dark.surfaceHover,
        onTertiaryContainer = dark.accentText,
        background = dark.bg,
        onBackground = dark.text,
        surface = dark.surface,
        onSurface = dark.text,
        surfaceVariant = dark.surfaceSoft,
        onSurfaceVariant = dark.textMuted,
        surfaceTint = dark.accent,
        inverseSurface = dark.text,
        inverseOnSurface = dark.bg,
        error = dark.danger,
        onError = dark.surface,
        errorContainer = dark.dangerBg,
        onErrorContainer = dark.danger,
        outline = dark.border,
        outlineVariant = dark.border,
        scrim = dark.backdrop,
        surfaceBright = dark.surface,
        surfaceDim = dark.surfaceSoft,
        surfaceContainerLowest = dark.surface,
        surfaceContainerLow = dark.surfaceSoft,
        surfaceContainer = dark.surfaceSoft,
        surfaceContainerHigh = dark.surfaceHover,
        surfaceContainerHighest = dark.border,
    )
}
