package com.justdrop.app.ui.components

import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Peer device card with presence indicator, platform icon, and trust badge.
 */
@Composable
fun DeviceCard(
    name: String,
    platform: String,
    address: String?,
    presence: String,
    trust: String,
    rssi: Int?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val presenceColor = when (presence) {
        "Available" -> Color(0xFF55EFC4)
        "Busy" -> Color(0xFFFF7675)
        "Receiving" -> Color(0xFFFDCB6E)
        else -> Color(0xFF636E72)
    }

    val platformIcon = when (platform) {
        "MacOS" -> Icons.Default.Laptop
        "Android" -> Icons.Default.PhoneAndroid
        "Windows" -> Icons.Default.DesktopWindows
        "Linux" -> Icons.Default.Terminal
        else -> Icons.Default.Devices
    }

    val trustBadge = when (trust) {
        "Favorite" -> "⭐"
        "Trusted" -> "✓"
        "Blocked" -> "🚫"
        else -> null
    }

    // Subtle pulse animation for available devices
    val infiniteTransition = rememberInfiniteTransition(label = "pulse")
    val pulseScale by infiniteTransition.animateFloat(
        initialValue = 1f,
        targetValue = if (presence == "Available") 1.05f else 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(1200, easing = EaseInOutSine),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "pulse_scale",
    )

    Card(
        modifier = modifier
            .fillMaxWidth()
            .scale(pulseScale)
            .clickable(onClick = onClick),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant,
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 2.dp),
    ) {
        Row(
            modifier = Modifier
                .padding(16.dp)
                .fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Platform icon with presence indicator
            Box {
                Surface(
                    modifier = Modifier.size(48.dp),
                    shape = CircleShape,
                    color = MaterialTheme.colorScheme.primaryContainer,
                ) {
                    Icon(
                        imageVector = platformIcon,
                        contentDescription = platform,
                        modifier = Modifier
                            .padding(12.dp)
                            .size(24.dp),
                        tint = MaterialTheme.colorScheme.onPrimaryContainer,
                    )
                }
                // Presence dot
                Box(
                    modifier = Modifier
                        .align(Alignment.BottomEnd)
                        .size(14.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.surface)
                        .padding(2.dp)
                        .clip(CircleShape)
                        .background(presenceColor),
                )
            }

            Spacer(modifier = Modifier.width(16.dp))

            // Device info
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = name,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (trustBadge != null) {
                        Spacer(modifier = Modifier.width(6.dp))
                        Text(text = trustBadge, fontSize = 14.sp)
                    }
                }
                Text(
                    text = buildString {
                        append(platform)
                        if (address != null) {
                            append(" • ")
                            append(address)
                        }
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            // Signal strength
            if (rssi != null) {
                SignalStrength(rssi = rssi)
            }

            Spacer(modifier = Modifier.width(8.dp))
            Icon(
                imageVector = Icons.Default.ChevronRight,
                contentDescription = "Send",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
fun SignalStrength(rssi: Int) {
    val bars = when {
        rssi > -50 -> 4
        rssi > -60 -> 3
        rssi > -70 -> 2
        rssi > -80 -> 1
        else -> 0
    }

    Row(
        horizontalArrangement = Arrangement.spacedBy(2.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        for (i in 0 until 4) {
            Box(
                modifier = Modifier
                    .width(4.dp)
                    .height((8 + i * 4).dp)
                    .clip(RoundedCornerShape(2.dp))
                    .background(
                        if (i < bars) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.outline.copy(alpha = 0.3f)
                    ),
            )
        }
    }
}
