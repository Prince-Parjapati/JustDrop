package com.justdrop.app.ui.screens

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.justdrop.app.ui.components.DeviceCard
import com.justdrop.app.ui.components.TransferProgressCard

/**
 * Main home screen showing nearby devices and active transfers.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    isServiceRunning: Boolean,
    devices: List<DeviceUiModel>,
    transfers: List<TransferUiModel>,
    onToggleService: () -> Unit,
    onDeviceClick: (String) -> Unit,
    onCancelTransfer: (String) -> Unit,
    onSettingsClick: () -> Unit,
) {
    Scaffold(
        topBar = {
            LargeTopAppBar(
                title = {
                    Column {
                        Text(
                            text = "JustDrop",
                            fontWeight = FontWeight.Bold,
                        )
                        Text(
                            text = if (isServiceRunning) "${devices.size} devices nearby"
                            else "Service stopped",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                },
                actions = {
                    // Power toggle
                    IconButton(onClick = onToggleService) {
                        Icon(
                            imageVector = if (isServiceRunning) Icons.Default.RadioButtonChecked
                            else Icons.Default.RadioButtonUnchecked,
                            contentDescription = "Toggle",
                            tint = if (isServiceRunning) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.outline,
                        )
                    }
                    IconButton(onClick = onSettingsClick) {
                        Icon(Icons.Default.Settings, contentDescription = "Settings")
                    }
                },
                colors = TopAppBarDefaults.largeTopAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                ),
            )
        },
    ) { padding ->
        LazyColumn(
            contentPadding = PaddingValues(
                top = padding.calculateTopPadding() + 8.dp,
                bottom = 16.dp,
                start = 16.dp,
                end = 16.dp,
            ),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // Active transfers
            if (transfers.isNotEmpty()) {
                item {
                    Text(
                        text = "Active Transfers",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.padding(bottom = 4.dp),
                    )
                }

                items(transfers, key = { it.id }) { transfer ->
                    TransferProgressCard(
                        peerName = transfer.peerName,
                        fileName = transfer.fileName,
                        direction = transfer.direction,
                        progress = transfer.progress,
                        speed = transfer.speed,
                        eta = transfer.eta,
                        onCancel = { onCancelTransfer(transfer.id) },
                    )
                }

                item { Spacer(modifier = Modifier.height(8.dp)) }
            }

            // Nearby devices header
            item {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = "Nearby Devices",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Spacer(modifier = Modifier.weight(1f))
                    if (isServiceRunning) {
                        ScanningIndicator()
                    }
                }
            }

            // Device list
            if (!isServiceRunning) {
                item {
                    EmptyState(
                        icon = Icons.Default.WifiOff,
                        title = "Service Not Running",
                        subtitle = "Turn on JustDrop to discover nearby devices",
                    )
                }
            } else if (devices.isEmpty()) {
                item {
                    EmptyState(
                        icon = Icons.Default.SearchOff,
                        title = "No Devices Found",
                        subtitle = "Make sure other devices have JustDrop running on the same network",
                    )
                }
            } else {
                items(devices, key = { it.deviceId }) { device ->
                    DeviceCard(
                        name = device.name,
                        platform = device.platform,
                        address = device.address,
                        presence = device.presence,
                        trust = device.trust,
                        rssi = device.rssi,
                        onClick = { onDeviceClick(device.deviceId) },
                    )
                }
            }
        }
    }
}

@Composable
fun ScanningIndicator() {
    val infiniteTransition = rememberInfiniteTransition(label = "scan")
    val rotation by infiniteTransition.animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(
            animation = tween(2000, easing = LinearEasing),
        ),
        label = "rotation",
    )

    Row(verticalAlignment = Alignment.CenterVertically) {
        Icon(
            imageVector = Icons.Default.Radar,
            contentDescription = "Scanning",
            modifier = Modifier
                .size(16.dp)
                .rotate(rotation),
            tint = MaterialTheme.colorScheme.primary,
        )
        Spacer(modifier = Modifier.width(4.dp))
        Text(
            text = "Scanning",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.primary,
        )
    }
}

@Composable
fun EmptyState(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    subtitle: String,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            modifier = Modifier.size(64.dp),
            tint = MaterialTheme.colorScheme.outline,
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = subtitle,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.outline,
        )
    }
}

// ── UI Models ──

data class DeviceUiModel(
    val deviceId: String,
    val name: String,
    val platform: String,
    val address: String?,
    val presence: String,
    val trust: String,
    val rssi: Int?,
)

data class TransferUiModel(
    val id: String,
    val peerName: String,
    val fileName: String,
    val direction: String,
    val progress: Float,
    val speed: String,
    val eta: String?,
)
