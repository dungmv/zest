use super::{hex_alpha_to_hsla, hex_to_hsla, AppTheme};

impl AppTheme {
    pub fn dark() -> Self {
        Self {
            // Modern Slate / Zinc Dark Theme: ~#15161c (soft, readable, comfortable)
            bg_app: hex_to_hsla(0x15161c),
            bg_titlebar: hex_to_hsla(0x181920),
            bg_sidebar: hex_to_hsla(0x16171e),
            bg_editor: hex_to_hsla(0x1a1b23),
            bg_response: hex_to_hsla(0x1a1b23),
            bg_panel: hex_to_hsla(0x1e1f29),
            bg_card: hex_to_hsla(0x22232e),
            bg_input: hex_to_hsla(0x1e1f29),
            bg_selected: hex_to_hsla(0x343849),
            bg_hover: hex_to_hsla(0x242632),
            bg_active: hex_to_hsla(0x393d50),

            // Fine, crisp borders
            border: hex_to_hsla(0x2a2b38),
            border_subtle: hex_to_hsla(0x22232f),
            border_focus: hex_to_hsla(0x3b82f6),

            // Typography
            text_primary: hex_to_hsla(0xcdd3df),
            text_secondary: hex_to_hsla(0xa7b2c4),
            text_muted: hex_to_hsla(0x98a2b4),
            text_disabled: hex_to_hsla(0x475569),
            sidebar_foreground: hex_to_hsla(0xb2bac9),
            icon_muted: hex_to_hsla(0x8b97aa),
            accent_text: hex_to_hsla(0x86aaf2),

            // Blue action fill with a readable, soft foreground
            primary: hex_to_hsla(0x2b67d5),
            primary_hover: hex_to_hsla(0x255dc4),
            primary_active: hex_to_hsla(0x214eaa),
            primary_foreground: hex_to_hsla(0xf3f5fa),

            // Secondary action
            secondary: hex_to_hsla(0x262836),
            secondary_hover: hex_to_hsla(0x313445),
            secondary_active: hex_to_hsla(0x3d4054),
            secondary_foreground: hex_to_hsla(0xb2bac9),

            // Danger action
            danger: hex_to_hsla(0xbd3644),
            danger_hover: hex_to_hsla(0xad2d3b),
            danger_active: hex_to_hsla(0x982534),
            danger_foreground: hex_to_hsla(0xf3f5fa),
            danger_text: hex_to_hsla(0xffa5aa),

            // Warning action
            warning: hex_to_hsla(0xf59e0b),
            warning_hover: hex_to_hsla(0xd97706),
            warning_foreground: hex_to_hsla(0x0f172a),

            // Success action
            success: hex_to_hsla(0x10b981),
            success_hover: hex_to_hsla(0x059669),
            success_foreground: hex_to_hsla(0xffffff),

            // Info action
            info: hex_to_hsla(0x06b6d4),
            info_hover: hex_to_hsla(0x0891b2),
            info_foreground: hex_to_hsla(0xffffff),

            // Git status
            git_added: hex_to_hsla(0x4ade80),
            git_modified: hex_to_hsla(0xfacc15),
            git_deleted: hex_to_hsla(0xef4444),
            git_untracked: hex_to_hsla(0x4ade80),
            git_ignored: hex_to_hsla(0x64748b),

            // Diff highlights
            diff_added_bg: hex_alpha_to_hsla(0x10b981, 0.15),
            diff_added_text: hex_to_hsla(0x4ade80),
            diff_deleted_bg: hex_alpha_to_hsla(0xef4444, 0.15),
            diff_deleted_text: hex_to_hsla(0xef4444),

            // Timing waterfall
            timing_dns: hex_to_hsla(0x06b6d4),
            timing_tcp: hex_to_hsla(0x3b82f6),
            timing_tls: hex_to_hsla(0xa855f7),
            timing_ttfb: hex_to_hsla(0xeab308),
            timing_download: hex_to_hsla(0x22c55e),

            // Console logs
            log_info: hex_to_hsla(0x86aaf2),
            log_warn: hex_to_hsla(0xdeb36e),
            log_error: hex_to_hsla(0xed7880),
            log_pre_request: hex_to_hsla(0x76b9cd),
            log_post_response: hex_to_hsla(0xb59adc),

            // Autocomplete variable tags
            tag_variable: hex_to_hsla(0x38bdf8),
            tag_global: hex_to_hsla(0xcfb2f3),
            tag_dynamic: hex_to_hsla(0xf59e0b),

            // Method colors
            method_get: hex_to_hsla(0x10b981),
            method_post: hex_to_hsla(0x79a5ef),
            method_put: hex_to_hsla(0xf59e0b),
            method_delete: hex_to_hsla(0xed7880),
            method_patch: hex_to_hsla(0xb59adc),
            method_head: hex_to_hsla(0x06b6d4),
            method_options: hex_to_hsla(0x98a2b4),
            method_websocket: hex_to_hsla(0xf97316),
            method_grpc: hex_to_hsla(0xec4899),

            // Status colors
            status_2xx: hex_to_hsla(0x10b981),
            status_3xx: hex_to_hsla(0x86aaf2),
            status_4xx: hex_to_hsla(0xf59e0b),
            status_5xx: hex_to_hsla(0xed7880),

            // Interpolation variable highlight
            var_resolved: hex_to_hsla(0x38bdf8),
            var_unresolved: hex_to_hsla(0xed7880),

            transparent: gpui_kit::transparent_black(),
        }
    }

    pub fn light() -> Self {
        Self {
            // Modern Light Theme: Slate 50 / White
            bg_app: hex_to_hsla(0xf8fafc),
            bg_titlebar: hex_to_hsla(0xf1f5f9),
            bg_sidebar: hex_to_hsla(0xf8fafc),
            bg_editor: hex_to_hsla(0xffffff),
            bg_response: hex_to_hsla(0xffffff),
            bg_panel: hex_to_hsla(0xf1f5f9),
            bg_card: hex_to_hsla(0xffffff),
            bg_input: hex_to_hsla(0xffffff),
            bg_selected: hex_to_hsla(0xe2e8f0),
            bg_hover: hex_to_hsla(0xf1f5f9),
            bg_active: hex_to_hsla(0xe2e8f0),

            // Fine, crisp borders for light mode
            border: hex_to_hsla(0xe2e8f0),
            border_subtle: hex_to_hsla(0xf1f5f9),
            border_focus: hex_to_hsla(0x2563eb),

            // Typography: ergonomic dark charcoal text, avoiding extreme black-on-white glare (Slate 800)
            text_primary: hex_to_hsla(0x1e293b),
            text_secondary: hex_to_hsla(0x475569),
            text_muted: hex_to_hsla(0x64748b),
            text_disabled: hex_to_hsla(0x94a3b8),
            sidebar_foreground: hex_to_hsla(0x1e293b),
            icon_muted: hex_to_hsla(0x64748b),
            accent_text: hex_to_hsla(0x2563eb),

            // Professional vibrant blue
            primary: hex_to_hsla(0x2563eb),
            primary_hover: hex_to_hsla(0x1d4ed8),
            primary_active: hex_to_hsla(0x1e40af),
            primary_foreground: hex_to_hsla(0xffffff),

            // Secondary action
            secondary: hex_to_hsla(0xf1f5f9),
            secondary_hover: hex_to_hsla(0xe2e8f0),
            secondary_active: hex_to_hsla(0xcbd5e1),
            secondary_foreground: hex_to_hsla(0x1e293b),

            // Danger action
            danger: hex_to_hsla(0xdc2626),
            danger_hover: hex_to_hsla(0xb91c1c),
            danger_active: hex_to_hsla(0x991b1b),
            danger_foreground: hex_to_hsla(0xffffff),
            danger_text: hex_to_hsla(0xe11d48),

            // Warning action
            warning: hex_to_hsla(0xd97706),
            warning_hover: hex_to_hsla(0xb45309),
            warning_foreground: hex_to_hsla(0xffffff),

            // Success action
            success: hex_to_hsla(0x059669),
            success_hover: hex_to_hsla(0x047857),
            success_foreground: hex_to_hsla(0xffffff),

            // Info action
            info: hex_to_hsla(0x0891b2),
            info_hover: hex_to_hsla(0x0e7490),
            info_foreground: hex_to_hsla(0xffffff),

            // Git status (high-contrast for light surfaces)
            git_added: hex_to_hsla(0x059669),
            git_modified: hex_to_hsla(0xd97706),
            git_deleted: hex_to_hsla(0xdc2626),
            git_untracked: hex_to_hsla(0x059669),
            git_ignored: hex_to_hsla(0x94a3b8),

            // Diff highlights (contrast-adjusted)
            diff_added_bg: hex_alpha_to_hsla(0x10b981, 0.12),
            diff_added_text: hex_to_hsla(0x059669),
            diff_deleted_bg: hex_alpha_to_hsla(0xef4444, 0.12),
            diff_deleted_text: hex_to_hsla(0xdc2626),

            // Timing waterfall
            timing_dns: hex_to_hsla(0x0891b2),
            timing_tcp: hex_to_hsla(0x2563eb),
            timing_tls: hex_to_hsla(0x9333ea),
            timing_ttfb: hex_to_hsla(0xd97706),
            timing_download: hex_to_hsla(0x16a34a),

            // Console logs
            log_info: hex_to_hsla(0x2563eb),
            log_warn: hex_to_hsla(0xd97706),
            log_error: hex_to_hsla(0xdc2626),
            log_pre_request: hex_to_hsla(0x0891b2),
            log_post_response: hex_to_hsla(0x9333ea),

            // Autocomplete variable tags
            tag_variable: hex_to_hsla(0x0284c7),
            tag_global: hex_to_hsla(0x7c3aed),
            tag_dynamic: hex_to_hsla(0xd97706),

            // Method colors for light mode (rich, solid contrast)
            method_get: hex_to_hsla(0x059669),
            method_post: hex_to_hsla(0x2563eb),
            method_put: hex_to_hsla(0xd97706),
            method_delete: hex_to_hsla(0xdc2626),
            method_patch: hex_to_hsla(0x7c3aed),
            method_head: hex_to_hsla(0x0891b2),
            method_options: hex_to_hsla(0x475569),
            method_websocket: hex_to_hsla(0xea580c),
            method_grpc: hex_to_hsla(0xdb2777),

            // Status colors
            status_2xx: hex_to_hsla(0x059669),
            status_3xx: hex_to_hsla(0x2563eb),
            status_4xx: hex_to_hsla(0xd97706),
            status_5xx: hex_to_hsla(0xdc2626),

            // Interpolation variable highlight
            var_resolved: hex_to_hsla(0x0284c7),
            var_unresolved: hex_to_hsla(0xdc2626),

            transparent: gpui_kit::transparent_black(),
        }
    }
}
