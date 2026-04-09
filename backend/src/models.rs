use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Clone, Serialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardResponse {
    pub sidebar: Sidebar,
    pub header: Header,
    pub page: Page,
    pub stats: Vec<StatCard>,
    pub players_overview: PlayersOverview,
    pub resource_chart: ResourceChart,
    pub distribution: Distribution,
    pub node_progress: Vec<NodeProgressItem>,
    pub table: ServerTable,
    pub quick_kpis: Vec<KpiCard>,
    pub node_locations: NodeLocations,
    pub activities: Vec<ActivityItem>,
    pub top_players: Vec<TopPlayer>,
    pub bandwidth: BandwidthChart,
    pub network_health: NetworkHealth,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sidebar {
    pub sections: Vec<SidebarSection>,
    pub user: SidebarUser,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarSection {
    pub label: String,
    pub items: Vec<SidebarItem>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarItem {
    pub label: String,
    pub icon: String,
    pub active: bool,
    pub badge: Option<String>,
    pub tooltip: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarUser {
    pub initials: String,
    pub name: String,
    pub role: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub search_placeholder: String,
    pub search_shortcut: String,
    pub live_label: String,
    pub profile_initials: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub title: String,
    pub subtitle: String,
    pub refresh_label: String,
    pub add_server_label: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatCard {
    pub label: String,
    pub value: String,
    pub change: String,
    pub change_direction: String,
    pub trend: String,
    pub color: String,
    pub icon: String,
    pub sparkline_id: String,
    pub sparkline_color: String,
    pub sparkline_data: Vec<f64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayersOverview {
    pub title: String,
    pub subtitle: String,
    pub tabs: Vec<String>,
    pub active_tab: String,
    pub export_label: String,
    pub labels: Vec<String>,
    pub data: Vec<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceChart {
    pub title: String,
    pub subtitle: String,
    pub labels: Vec<String>,
    pub cpu: Vec<u32>,
    pub ram: Vec<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Distribution {
    pub title: String,
    pub subtitle: String,
    pub total: String,
    pub total_label: String,
    pub labels: Vec<String>,
    pub values: Vec<u32>,
    pub colors: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeProgressItem {
    pub name: String,
    pub value: String,
    pub width: String,
    pub background: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTable {
    pub title: String,
    pub subtitle: String,
    pub search_placeholder: String,
    pub game_options: Vec<String>,
    pub status_options: Vec<String>,
    pub rows: Vec<ServerRow>,
    pub pagination: Pagination,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRow {
    pub name: String,
    pub ip: String,
    pub uuid: String,
    pub dot: String,
    pub game: Badge,
    pub status: Badge,
    pub players: String,
    pub cpu: ProgressData,
    pub ram: ProgressData,
    pub region: Badge,
    pub actions: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub label: String,
    pub class_name: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressData {
    pub width: String,
    pub background: String,
    pub value: String,
    pub muted: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub summary: String,
    pub pages: Vec<String>,
    pub active: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KpiCard {
    pub label: String,
    pub value: String,
    pub sub: String,
    pub color: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeLocations {
    pub title: String,
    pub nodes: Vec<MapNode>,
    pub footer: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapNode {
    pub label: String,
    pub background: String,
    pub left: String,
    pub top: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub icon: String,
    pub icon_style: StyleDescriptor,
    pub text: ActivityText,
    pub badge: ActivityBadge,
    pub time: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleDescriptor {
    pub background: String,
    pub color: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityText {
    pub before: String,
    pub strong: String,
    pub after: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBadge {
    pub label: String,
    pub class_name: String,
    pub style: FontSizeStyle,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontSizeStyle {
    pub font_size: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopPlayer {
    pub initials: String,
    pub background: String,
    pub name: String,
    pub server: String,
    pub time: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BandwidthChart {
    pub title: String,
    pub labels: Vec<String>,
    pub data: Vec<u32>,
    pub colors: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkHealth {
    pub title: String,
    pub regions: Vec<NetworkRegion>,
    pub stats: Vec<NetworkStat>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRegion {
    pub name: String,
    pub value: String,
    pub width: String,
    pub color: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStat {
    pub value: String,
    pub label: String,
    pub color: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddServerRequest {
    pub name: String,
    pub ip: String,
    pub rcon_port: u16,
    pub rcon_password: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateServerRequest {
    pub name: String,
    pub ip: String,
    pub rcon_port: u16,
    pub rcon_password: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResponse {
    pub message: String,
    pub server_uuid: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedServerDetailResponse {
    pub name: String,
    pub ip: String,
    pub rcon_port: u16,
    pub rcon_password: String,
    pub server_uuid: String,
    pub status_label: String,
    pub agent_id: Option<String>,
    pub agent_online: bool,
    pub workspace_roots: Vec<WorkspaceRootSummary>,
    pub primary_log_path: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAgentAuthResponse {
    pub server_uuid: String,
    pub has_key: bool,
    pub key_preview: Option<String>,
    pub plain_key: Option<String>,
    pub rotated_at: Option<u64>,
    pub agent_online: bool,
    pub agent_id: Option<String>,
    pub last_heartbeat_at: Option<u64>,
    pub workspace_roots: Vec<WorkspaceRootSummary>,
    pub primary_log_path: String,
}

#[derive(Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ServerFeatureFlagsResponse {
    pub server_uuid: String,
    pub disable_vehicle_claiming: bool,
    pub force_all_vehicle_availability: bool,
    pub force_all_deployable_availability: bool,
    pub force_all_role_availability: bool,
    pub disable_vehicle_team_requirement: bool,
    pub disable_vehicle_kit_requirement: bool,
    pub no_respawn_timer: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateServerFeatureFlagRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseRuleKind {
    Regex,
}

impl Default for ParseRuleKind {
    fn default() -> Self {
        Self::Regex
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseRule {
    pub id: String,
    #[serde(default)]
    pub kind: ParseRuleKind,
    pub pattern: String,
    pub event_type: String,
    pub severity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateServerParseRulesRequest {
    pub rules: Vec<ParseRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerParseRulesResponse {
    pub server_uuid: String,
    pub version: Option<u64>,
    pub rules: Vec<ParseRule>,
    pub agent_online: bool,
    pub agent_id: Option<String>,
    pub last_heartbeat_at: Option<u64>,
    pub applied: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedLogEvent {
    pub agent_id: String,
    pub rule_id: String,
    pub event_type: String,
    pub severity: String,
    pub source: String,
    pub cursor: String,
    pub line_number: u64,
    pub raw_line: String,
    pub observed_at: String,
    pub payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentParsedEvents {
    pub events: Vec<ParsedLogEvent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedEventQuery {
    pub event_type: Option<String>,
    pub limit: Option<u32>,
    pub before: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerParsedEventsResponse {
    pub server_uuid: String,
    pub event_type: Option<String>,
    pub items: Vec<ParsedLogEvent>,
    pub next_before: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAgentSummary {
    pub agent_id: String,
    pub server_uuid: String,
    pub platform: AgentPlatform,
    pub version: String,
    pub workspace_roots: Vec<WorkspaceRootSummary>,
    pub primary_log_path: String,
    pub connected_at: u64,
    pub last_heartbeat_at: u64,
}

#[derive(Clone, Serialize)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Clone, FromRow)]
pub struct ManagedServer {
    pub name: String,
    pub ip: String,
    pub rcon_port: i32,
    pub server_uuid: String,
    #[allow(dead_code)]
    pub rcon_password: String,
}

impl DashboardResponse {
    pub fn from_servers(servers: &[ManagedServer]) -> Self {
        let server_count = servers.len();
        let server_rows = servers
            .iter()
            .map(ServerRow::from_server)
            .collect::<Vec<_>>();
        let server_count_text = server_count.to_string();

        Self {
            sidebar: Sidebar {
                sections: vec![
                    SidebarSection {
                        label: "概览".to_string(),
                        items: vec![
                            SidebarItem {
                                label: "仪表盘".to_string(),
                                icon: "grid".to_string(),
                                active: true,
                                badge: None,
                                tooltip: None,
                            },
                            SidebarItem {
                                label: "数据分析".to_string(),
                                icon: "analytics".to_string(),
                                active: false,
                                badge: None,
                                tooltip: None,
                            },
                        ],
                    },
                    SidebarSection {
                        label: "服务器".to_string(),
                        items: vec![
                            SidebarItem {
                                label: "服务器管理".to_string(),
                                icon: "server-manager".to_string(),
                                active: false,
                                badge: Some(server_count_text.clone()),
                                tooltip: Some("服务器".to_string()),
                            },
                            SidebarItem {
                                label: "节点".to_string(),
                                icon: "globe".to_string(),
                                active: false,
                                badge: None,
                                tooltip: None,
                            },
                            SidebarItem {
                                label: "备份".to_string(),
                                icon: "cube".to_string(),
                                active: false,
                                badge: None,
                                tooltip: None,
                            },
                        ],
                    },
                    SidebarSection {
                        label: "管理".to_string(),
                        items: vec![
                            SidebarItem {
                                label: "玩家".to_string(),
                                icon: "players".to_string(),
                                active: false,
                                badge: None,
                                tooltip: None,
                            },
                            SidebarItem {
                                label: "安全".to_string(),
                                icon: "shield".to_string(),
                                active: false,
                                badge: None,
                                tooltip: None,
                            },
                            SidebarItem {
                                label: "日志".to_string(),
                                icon: "file".to_string(),
                                active: false,
                                badge: None,
                                tooltip: None,
                            },
                            SidebarItem {
                                label: "网络".to_string(),
                                icon: "network".to_string(),
                                active: false,
                                badge: None,
                                tooltip: None,
                            },
                        ],
                    },
                    SidebarSection {
                        label: "系统".to_string(),
                        items: vec![
                            SidebarItem {
                                label: "通知".to_string(),
                                icon: "mail".to_string(),
                                active: false,
                                badge: Some("0".to_string()),
                                tooltip: None,
                            },
                            SidebarItem {
                                label: "设置".to_string(),
                                icon: "settings".to_string(),
                                active: false,
                                badge: None,
                                tooltip: None,
                            },
                        ],
                    },
                ],
                user: SidebarUser {
                    initials: "SA".to_string(),
                    name: "超级管理员".to_string(),
                    role: "系统管理员".to_string(),
                },
            },
            header: Header {
                search_placeholder: "搜索服务器、玩家、日志…".to_string(),
                search_shortcut: "⌘K".to_string(),
                live_label: "已连接".to_string(),
                profile_initials: "SA".to_string(),
            },
            page: Page {
                title: "仪表盘".to_string(),
                subtitle: "游戏服务器基础设施总览".to_string(),
                refresh_label: "刷新".to_string(),
                add_server_label: "添加服务器".to_string(),
            },
            stats: vec![
                StatCard {
                    label: "服务器总数".to_string(),
                    value: server_count_text.clone(),
                    change: "已接入".to_string(),
                    change_direction: "neutral".to_string(),
                    trend: format!("当前已接入 {} 台服务器", server_count),
                    color: "indigo".to_string(),
                    icon: "server-manager".to_string(),
                    sparkline_id: "spark1".to_string(),
                    sparkline_color: "#6366f1".to_string(),
                    sparkline_data: vec![],
                },
                empty_stat("在线玩家", "green", "players", "spark2", "#10b981"),
                empty_stat("平均 CPU 占用", "orange", "cpu", "spark3", "#f97316"),
                empty_stat("已用带宽", "purple", "bandwidth", "spark4", "#8b5cf6"),
                empty_stat("运行时间", "blue", "analytics", "spark5", "#3b82f6"),
                empty_stat("活跃事件", "red", "alert", "spark6", "#ef4444"),
            ],
            players_overview: PlayersOverview {
                title: "在线玩家 — 24 小时概览".to_string(),
                subtitle: "全服实时并发玩家趋势".to_string(),
                tabs: vec!["24小时".to_string(), "7天".to_string(), "30天".to_string()],
                active_tab: "24小时".to_string(),
                export_label: "导出".to_string(),
                labels: vec![],
                data: vec![],
            },
            resource_chart: ResourceChart {
                title: "CPU 与内存占用".to_string(),
                subtitle: "基础设施资源使用情况".to_string(),
                labels: vec![],
                cpu: vec![],
                ram: vec![],
            },
            distribution: Distribution {
                title: "游戏类型分布".to_string(),
                subtitle: "各游戏活跃服务器数量".to_string(),
                total: server_count_text.clone(),
                total_label: "台服务器".to_string(),
                labels: vec![],
                values: vec![],
                colors: vec![],
            },
            node_progress: vec![],
            table: ServerTable {
                title: "服务器管理".to_string(),
                subtitle: "管理并监控所有游戏服务器".to_string(),
                search_placeholder: "搜索服务器…".to_string(),
                game_options: vec!["全部游戏".to_string()],
                status_options: vec!["全部状态".to_string(), "在线".to_string()],
                rows: server_rows,
                pagination: Pagination {
                    summary: format!("当前显示 {} / {} 台服务器", server_count, server_count),
                    pages: vec!["1".to_string()],
                    active: "1".to_string(),
                },
            },
            quick_kpis: vec![
                KpiCard {
                    label: "平均延迟".to_string(),
                    value: "--".to_string(),
                    sub: "等待后端数据".to_string(),
                    color: Some("var(--green)".to_string()),
                },
                KpiCard {
                    label: "丢包率".to_string(),
                    value: "--".to_string(),
                    sub: "等待后端数据".to_string(),
                    color: Some("var(--accent)".to_string()),
                },
                KpiCard {
                    label: "Tick 速率".to_string(),
                    value: "--".to_string(),
                    sub: "等待后端数据".to_string(),
                    color: None,
                },
                KpiCard {
                    label: "拦截 DDoS".to_string(),
                    value: "--".to_string(),
                    sub: "等待后端数据".to_string(),
                    color: Some("var(--red)".to_string()),
                },
            ],
            node_locations: NodeLocations {
                title: "节点位置".to_string(),
                nodes: vec![],
                footer: "0 个节点 · 0 个地区".to_string(),
            },
            activities: vec![],
            top_players: vec![],
            bandwidth: BandwidthChart {
                title: "各服务器带宽".to_string(),
                labels: vec![],
                data: vec![],
                colors: vec![],
            },
            network_health: NetworkHealth {
                title: "网络健康度".to_string(),
                regions: vec![],
                stats: vec![
                    NetworkStat {
                        value: "--".to_string(),
                        label: "服务可用性".to_string(),
                        color: "var(--green)".to_string(),
                    },
                    NetworkStat {
                        value: "--".to_string(),
                        label: "丢包率".to_string(),
                        color: "var(--accent)".to_string(),
                    },
                ],
            },
        }
    }
}

impl ServerRow {
    fn from_server(server: &ManagedServer) -> Self {
        Self {
            name: server.name.clone(),
            ip: format!("{}:{}", server.ip, server.rcon_port),
            uuid: server.server_uuid.clone(),
            dot: "online".to_string(),
            game: Badge {
                label: "未识别".to_string(),
                class_name: "badge badge-gray".to_string(),
            },
            status: Badge {
                label: "● 在线".to_string(),
                class_name: "badge badge-green".to_string(),
            },
            players: "-- / --".to_string(),
            cpu: ProgressData {
                width: "0%".to_string(),
                background: "var(--text-muted)".to_string(),
                value: "--".to_string(),
                muted: true,
            },
            ram: ProgressData {
                width: "0%".to_string(),
                background: "var(--text-muted)".to_string(),
                value: "--".to_string(),
                muted: true,
            },
            region: Badge {
                label: "--".to_string(),
                class_name: "badge badge-gray".to_string(),
            },
            actions: vec![
                "manage".to_string(),
                "edit".to_string(),
                "delete".to_string(),
            ],
        }
    }
}

impl ManagedServerDetailResponse {
    pub fn from_server(
        server: &ManagedServer,
        binding_agent_id: Option<&str>,
        online_agent: Option<&OnlineAgent>,
    ) -> Self {
        Self {
            name: server.name.clone(),
            ip: server.ip.clone(),
            rcon_port: server.rcon_port as u16,
            rcon_password: server.rcon_password.clone(),
            server_uuid: server.server_uuid.clone(),
            status_label: "● 在线".to_string(),
            agent_id: binding_agent_id.map(ToOwned::to_owned),
            agent_online: online_agent.is_some(),
            workspace_roots: online_agent
                .map(|agent| agent.registration.workspace_roots.clone())
                .unwrap_or_default(),
            primary_log_path: online_agent
                .map(|agent| agent.registration.primary_log_path.clone())
                .unwrap_or_default(),
        }
    }
}

impl ServerAgentAuthResponse {
    pub fn from_auth(
        server_uuid: &str,
        key_preview: Option<String>,
        rotated_at: Option<u64>,
        plain_key: Option<String>,
        online_agent: Option<&OnlineAgent>,
    ) -> Self {
        Self {
            server_uuid: server_uuid.to_string(),
            has_key: key_preview.is_some(),
            key_preview,
            plain_key,
            rotated_at,
            agent_online: online_agent.is_some(),
            agent_id: online_agent.map(|agent| agent.registration.agent_id.clone()),
            last_heartbeat_at: online_agent.map(|agent| agent.last_heartbeat_at_ms),
            workspace_roots: online_agent
                .map(|agent| agent.registration.workspace_roots.clone())
                .unwrap_or_default(),
            primary_log_path: online_agent
                .map(|agent| agent.registration.primary_log_path.clone())
                .unwrap_or_default(),
        }
    }
}

impl ServerFeatureFlagsResponse {
    pub fn all_disabled(server_uuid: &str) -> Self {
        Self {
            server_uuid: server_uuid.to_string(),
            disable_vehicle_claiming: false,
            force_all_vehicle_availability: false,
            force_all_deployable_availability: false,
            force_all_role_availability: false,
            disable_vehicle_team_requirement: false,
            disable_vehicle_kit_requirement: false,
            no_respawn_timer: false,
        }
    }

    pub fn set_feature_enabled(
        &mut self,
        feature_key: &str,
        enabled: bool,
    ) -> Result<&'static str, String> {
        match feature_key {
            "disableVehicleClaiming" => {
                self.disable_vehicle_claiming = enabled;
                Ok("AdminDisableVehicleClaiming")
            }
            "forceAllVehicleAvailability" => {
                self.force_all_vehicle_availability = enabled;
                Ok("AdminForceAllVehicleAvailability")
            }
            "forceAllDeployableAvailability" => {
                self.force_all_deployable_availability = enabled;
                Ok("AdminForceAllDeployableAvailability")
            }
            "forceAllRoleAvailability" => {
                self.force_all_role_availability = enabled;
                Ok("AdminForceAllRoleAvailability")
            }
            "disableVehicleTeamRequirement" => {
                self.disable_vehicle_team_requirement = enabled;
                Ok("AdminDisableVehicleTeamRequirement")
            }
            "disableVehicleKitRequirement" => {
                self.disable_vehicle_kit_requirement = enabled;
                Ok("AdminDisableVehicleKitRequirement")
            }
            "noRespawnTimer" => {
                self.no_respawn_timer = enabled;
                Ok("AdminNoRespawnTimer")
            }
            _ => Err("未知的服务器功能开关".to_string()),
        }
    }

    pub fn to_rcon_commands(&self) -> [String; 7] {
        [
            format!(
                "AdminDisableVehicleClaiming {}",
                bool_to_rcon_flag(self.disable_vehicle_claiming)
            ),
            format!(
                "AdminForceAllVehicleAvailability {}",
                bool_to_rcon_flag(self.force_all_vehicle_availability)
            ),
            format!(
                "AdminForceAllDeployableAvailability {}",
                bool_to_rcon_flag(self.force_all_deployable_availability)
            ),
            format!(
                "AdminForceAllRoleAvailability {}",
                bool_to_rcon_flag(self.force_all_role_availability)
            ),
            format!(
                "AdminDisableVehicleTeamRequirement {}",
                bool_to_rcon_flag(self.disable_vehicle_team_requirement)
            ),
            format!(
                "AdminDisableVehicleKitRequirement {}",
                bool_to_rcon_flag(self.disable_vehicle_kit_requirement)
            ),
            format!(
                "AdminNoRespawnTimer {}",
                bool_to_rcon_flag(self.no_respawn_timer)
            ),
        ]
    }
}

impl ServerParseRulesResponse {
    pub fn from_rules(
        server_uuid: &str,
        version: Option<u64>,
        rules: Vec<ParseRule>,
        online_agent: Option<&OnlineAgent>,
        applied: bool,
        message: impl Into<String>,
    ) -> Self {
        Self {
            server_uuid: server_uuid.to_string(),
            version,
            rules,
            agent_online: online_agent.is_some(),
            agent_id: online_agent.map(|agent| agent.registration.agent_id.clone()),
            last_heartbeat_at: online_agent.map(|agent| agent.last_heartbeat_at_ms),
            applied,
            message: message.into(),
        }
    }
}

impl ServerParsedEventsResponse {
    pub fn from_items(
        server_uuid: &str,
        event_type: Option<String>,
        items: Vec<ParsedLogEvent>,
    ) -> Self {
        let next_before = items
            .last()
            .and_then(|item| item.observed_at.parse::<u64>().ok());

        Self {
            server_uuid: server_uuid.to_string(),
            event_type,
            items,
            next_before,
        }
    }
}

fn empty_stat(
    label: &str,
    color: &str,
    icon: &str,
    sparkline_id: &str,
    sparkline_color: &str,
) -> StatCard {
    StatCard {
        label: label.to_string(),
        value: "--".to_string(),
        change: "待接入".to_string(),
        change_direction: "neutral".to_string(),
        trend: "等待后端返回数据".to_string(),
        color: color.to_string(),
        icon: icon.to_string(),
        sparkline_id: sparkline_id.to_string(),
        sparkline_color: sparkline_color.to_string(),
        sparkline_data: vec![],
    }
}

fn bool_to_rcon_flag(value: bool) -> u8 {
    if value { 1 } else { 0 }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRootSummary {
    pub name: String,
    pub logical_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentPlatform {
    Linux,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEnvelope {
    pub agent_id: String,
    pub source: String,
    pub cursor: String,
    pub line_number: u64,
    pub raw_line: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLogChunk {
    pub entries: Vec<LogEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentFileChanged {
    pub logical_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRegistration {
    pub server_uuid: String,
    pub agent_id: String,
    pub auth_key: String,
    pub platform: AgentPlatform,
    pub version: String,
    pub workspace_roots: Vec<WorkspaceRootSummary>,
    pub primary_log_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRegistered {
    pub agent_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentHeartbeat {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTreeEntry {
    pub logical_path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTreeResult {
    pub entries: Vec<FileTreeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReadRequest {
    pub logical_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReadResult {
    pub logical_path: String,
    pub content: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTreeRequest {
    pub logical_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWriteRequest {
    pub logical_path: String,
    pub content: String,
    pub expected_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceParseRulesRequest {
    pub version: u64,
    pub rules: Vec<ParseRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceParseRulesResult {
    pub version: u64,
    pub rule_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWriteRequestBody {
    pub logical_path: String,
    pub content: String,
    pub expected_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteResult {
    pub logical_path: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilePathQuery {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AgentCommand {
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "file.tree")]
    FileTree(FileTreeRequest),
    #[serde(rename = "file.read")]
    FileRead(FileReadRequest),
    #[serde(rename = "file.write")]
    FileWrite(FileWriteRequest),
    #[serde(rename = "parseRules.replace")]
    ReplaceParseRules(ReplaceParseRulesRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCommandEnvelope {
    pub request_id: String,
    pub command: AgentCommand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCommandResult {
    pub request_id: String,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AgentStreamEvent {
    #[serde(rename = "agent.logChunk")]
    LogChunk(AgentLogChunk),
    #[serde(rename = "agent.fileChanged")]
    FileChanged(AgentFileChanged),
    #[serde(rename = "agent.parsedEvents")]
    ParsedEvents(AgentParsedEvents),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AgentClientMessage {
    #[serde(rename = "agent.register")]
    Register(AgentRegistration),
    #[serde(rename = "agent.heartbeat")]
    Heartbeat(AgentHeartbeat),
    #[serde(rename = "agent.commandResult")]
    CommandResult(AgentCommandResult),
    #[serde(rename = "agent.logChunk")]
    LogChunk(AgentLogChunk),
    #[serde(rename = "agent.fileChanged")]
    FileChanged(AgentFileChanged),
    #[serde(rename = "agent.parsedEvents")]
    ParsedEvents(AgentParsedEvents),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AgentServerMessage {
    #[serde(rename = "agent.registered")]
    Registered(AgentRegistered),
    #[serde(rename = "agent.command")]
    Command(AgentCommandEnvelope),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAgent {
    pub session_id: String,
    pub connected_at_ms: u64,
    pub last_heartbeat_at_ms: u64,
    pub registration: AgentRegistration,
}
