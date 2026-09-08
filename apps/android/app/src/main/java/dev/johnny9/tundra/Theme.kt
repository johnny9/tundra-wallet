package dev.johnny9.tundra

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp

private val Dark = darkColorScheme(
    primary = Color(0xFFF89B2A), onPrimary = Color(0xFF171717),
    background = Color(0xFF0E1115), surface = Color(0xFF0E1115),
    surfaceVariant = Color(0xFF1B2129), onSurface = Color(0xFFF2F5F8),
    onSurfaceVariant = Color(0xFFADB7C3), outline = Color(0xFF637080)
)
private val Light = lightColorScheme(
    primary = Color(0xFF975000), onPrimary = Color.White,
    background = Color(0xFFFBFCFD), surface = Color(0xFFFBFCFD),
    surfaceVariant = Color(0xFFF0F3F6), onSurface = Color(0xFF17202A),
    onSurfaceVariant = Color(0xFF566372), outline = Color(0xFF85929F)
)
@Composable fun TundraTheme(dark: Boolean = true, content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = if (dark) Dark else Light, content = content)
}
@Composable fun TundraMark() {
    val color = MaterialTheme.colorScheme.primary
    Canvas(Modifier.size(28.dp)) {
        val u = size.width / 24
        val path = Path().apply {
            moveTo(3*u, 12*u); lineTo(7*u, 12*u); lineTo(11*u, 7*u)
            lineTo(15*u, 12*u); lineTo(21*u, 12*u)
        }
        drawPath(path, color, style = Stroke(1.8f*u, cap = StrokeCap.Round))
        drawLine(color, Offset(5*u,17*u),Offset(19*u,17*u),1.8f*u,StrokeCap.Round)
        drawLine(color, Offset(8*u,21*u),Offset(16*u,21*u),1.8f*u,StrokeCap.Round)
    }
}
