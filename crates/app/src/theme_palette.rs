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
            text_secondary: hex_to_hsla(0x94a3b8),
            text_muted: hex_to_hsla(0x8791a3),
            text_disabled: hex_to_hsla(0x475569),
            sidebar_foreground: hex_to_hsla(0xb2bac9),
            sidebar_icon: hex_to_hsla(0x8b97aa),

            // Professional vibrant blue
            primary: hex_to_hsla(0x3b82f6),
            primary_hover: hex_to_hsla(0x2563eb),
            primary_active: hex_to_hsla(0x1d4ed8),
            primary_foreground: hex_to_hsla(0xffffff),

            // Secondary action
            secondary: hex_to_hsla(0x262836),
            secondary_hover: hex_to_hsla(0x313445),
            secondary_active: hex_to_hsla(0x3d4054),
            secondary_foreground: hex_to_hsla(0xcdd3df),

            // Danger action
            danger: hex_to_hsla(0xef4444),
            danger_hover: hex_to_hsla(0xdc2626),
            danger_active: hex_to_hsla(0xb91c1c),
            danger_foreground: hex_to_hsla(0xffffff),

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
            log_info: hex_to_hsla(0x3b82f6),
            log_warn: hex_to_hsla(0xf59e0b),
            log_error: hex_to_hsla(0xef4444),
            log_pre_request: hex_to_hsla(0x06b6d4),
            log_post_response: hex_to_hsla(0xa855f7),

            // Autocomplete variable tags
            tag_variable: hex_to_hsla(0x38bdf8),
            tag_dynamic: hex_to_hsla(0xf59e0b),

            // Method colors
            method_get: hex_to_hsla(0x10b981),
            method_post: hex_to_hsla(0x3b82f6),
            method_put: hex_to_hsla(0xf59e0b),
            method_delete: hex_to_hsla(0xef4444),
            method_patch: hex_to_hsla(0x8b5cf6),
            method_head: hex_to_hsla(0x06b6d4),
            method_options: hex_to_hsla(0x64748b),
            method_websocket: hex_to_hsla(0xf97316),
            method_grpc: hex_to_hsla(0xec4899),

            // Status colors
            status_2xx: hex_to_hsla(0x10b981),
            status_3xx: hex_to_hsla(0x3b82f6),
            status_4xx: hex_to_hsla(0xf59e0b),
            status_5xx: hex_to_hsla(0xef4444),

            // Interpolation variable highlight
            var_resolved: hex_to_hsla(0x38bdf8),
            var_unresolved: hex_to_hsla(0xef4444),

            // JSON Syntax Highlighting
            json_key: hex_to_hsla(0x38bdf8),         // Sky blue
            json_string: hex_to_hsla(0x34d399),      // Emerald green
            json_number: hex_to_hsla(0xfbbf24),      // Amber / Gold
            json_boolean: hex_to_hsla(0xc084fc),     // Purple / Violet
            json_null: hex_to_hsla(0xf87171),        // Soft coral red
            json_punctuation: hex_to_hsla(0x94a3b8), // Slate / Muted gray

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
            sidebar_icon: hex_to_hsla(0x64748b),

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

            // JSON Syntax Highlighting for light backgrounds
            json_key: hex_to_hsla(0x0284c7),         // Rich Blue/Cyan
            json_string: hex_to_hsla(0x059669),      // Deep Emerald
            json_number: hex_to_hsla(0xd97706),      // Warm Amber
            json_boolean: hex_to_hsla(0x7c3aed),     // Violet
            json_null: hex_to_hsla(0xe11d48),        // Rose Red
            json_punctuation: hex_to_hsla(0x64748b), // Slate gray

            transparent: gpui_kit::transparent_black(),
        }
    }
}
