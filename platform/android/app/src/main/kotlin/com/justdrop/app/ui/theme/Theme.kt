package com.justdrop.app.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

// JustDrop brand colors
val JustDropPrimary = Color(0xFF6C5CE7)
val JustDropSecondary = Color(0xFF00B894)
val JustDropTertiary = Color(0xFFFD79A8)
val JustDropError = Color(0xFFD63031)

private val DarkColorScheme =
    darkColorScheme(
        primary = Color(0xFFA29BFE),
        onPrimary = Color(0xFF1A1A2E),
        primaryContainer = Color(0xFF4834D4),
        onPrimaryContainer = Color(0xFFE8E4FF),
        secondary = Color(0xFF55EFC4),
        onSecondary = Color(0xFF003B30),
        secondaryContainer = Color(0xFF00B894),
        onSecondaryContainer = Color(0xFFD5FFF1),
        tertiary = Color(0xFFFD79A8),
        onTertiary = Color(0xFF3B0020),
        error = JustDropError,
        background = Color(0xFF0F0F1A),
        onBackground = Color(0xFFE8E8F0),
        surface = Color(0xFF1A1A2E),
        onSurface = Color(0xFFE8E8F0),
        surfaceVariant = Color(0xFF2D2D44),
        onSurfaceVariant = Color(0xFFCACAD8),
        outline = Color(0xFF4A4A66),
    )

private val LightColorScheme =
    lightColorScheme(
        primary = JustDropPrimary,
        onPrimary = Color.White,
        primaryContainer = Color(0xFFE8E4FF),
        onPrimaryContainer = Color(0xFF1A0066),
        secondary = JustDropSecondary,
        onSecondary = Color.White,
        secondaryContainer = Color(0xFFD5FFF1),
        onSecondaryContainer = Color(0xFF003B30),
        tertiary = JustDropTertiary,
        error = JustDropError,
        background = Color(0xFFF8F8FF),
        onBackground = Color(0xFF1A1A2E),
        surface = Color.White,
        onSurface = Color(0xFF1A1A2E),
        surfaceVariant = Color(0xFFF0F0F8),
        onSurfaceVariant = Color(0xFF4A4A66),
        outline = Color(0xFFCACAD8),
    )

@Composable
fun JustDropTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    dynamicColor: Boolean = true,
    content: @Composable () -> Unit,
) {
    val colorScheme =
        when {
            dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
                val context = LocalContext.current
                if (darkTheme) {
                    dynamicDarkColorScheme(context)
                } else {
                    dynamicLightColorScheme(context)
                }
            }

            darkTheme -> {
                DarkColorScheme
            }

            else -> {
                LightColorScheme
            }
        }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        content = content,
    )
}

val Typography =
    Typography(
        headlineLarge =
            androidx.compose.ui.text.TextStyle(
                fontWeight = FontWeight.Bold,
                fontSize = 28.sp,
                letterSpacing = (-0.5).sp,
            ),
        headlineMedium =
            androidx.compose.ui.text.TextStyle(
                fontWeight = FontWeight.SemiBold,
                fontSize = 22.sp,
            ),
        titleLarge =
            androidx.compose.ui.text.TextStyle(
                fontWeight = FontWeight.SemiBold,
                fontSize = 18.sp,
            ),
        bodyLarge =
            androidx.compose.ui.text.TextStyle(
                fontWeight = FontWeight.Normal,
                fontSize = 16.sp,
            ),
        labelLarge =
            androidx.compose.ui.text.TextStyle(
                fontWeight = FontWeight.Medium,
                fontSize = 14.sp,
            ),
    )
