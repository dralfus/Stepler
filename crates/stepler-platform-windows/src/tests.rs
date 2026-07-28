use super::*;
use stepler_platform::HotkeyListener;

#[test]
fn skeleton_hotkey_listener_tracks_running_state() {
    let mut listener = WindowsHotkeyListener::default();

    assert!(!listener.is_running());

    listener.start().unwrap();
    assert!(listener.is_running());

    listener.stop().unwrap();
    assert!(!listener.is_running());
}

#[test]
fn context_id_parser_rejects_invalid_ids() {
    assert_eq!(parse_hwnd_id("nope"), None);
    assert_eq!(parse_hwnd_id("hwnd:"), None);
    assert_eq!(parse_hwnd_id("hwnd:XYZ"), None);
    assert_eq!(parse_hwnd_id("hwnd:1A"), Some(0x1A));
}

#[cfg(windows)]
#[test]
fn test_foreground_override_parses_decimal_and_hex_hwnds() {
    unsafe {
        std::env::set_var("STEPLER_TEST_FOREGROUND_HWND", "123");
    }
    assert_eq!(test_foreground_hwnd_override(), Some(123));
    unsafe {
        std::env::set_var("STEPLER_TEST_FOREGROUND_HWND", "0x7B");
    }
    assert_eq!(test_foreground_hwnd_override(), Some(123));
    unsafe {
        std::env::remove_var("STEPLER_TEST_FOREGROUND_HWND");
    }
}

#[test]
fn supported_edit_class_is_allowlisted() {
    assert!(is_supported_edit_class("Edit"));
    assert!(is_supported_edit_class("RICHEDIT50W"));
    assert!(is_supported_edit_class("RichEditD2DPT"));
    assert!(!is_supported_edit_class("ConsoleWindowClass"));
    assert!(!is_supported_edit_class("Notepad"));
}

#[test]
fn supported_terminal_class_is_allowlisted() {
    assert!(is_supported_terminal_class(
        "CASCADIA_HOSTING_WINDOW_CLASS",
        "Windows.UI.Input.InputSite.WindowClass"
    ));
    assert!(is_supported_terminal_class(
        "ConsoleWindowClass",
        "ConsoleWindowClass"
    ));
    assert!(!is_supported_terminal_class("Notepad", "Edit"));
    assert!(!is_supported_terminal_class(
        "ApplicationFrameWindow",
        "Windows.UI.Input.InputSite.WindowClass"
    ));
}

#[test]
fn classic_console_class_requires_foreground_and_focus_console() {
    assert!(is_classic_console_class(
        "ConsoleWindowClass",
        "ConsoleWindowClass"
    ));
    assert!(!is_classic_console_class(
        "CASCADIA_HOSTING_WINDOW_CLASS",
        "Windows.UI.Input.InputSite.WindowClass"
    ));
}

#[test]
fn classic_console_is_not_psreadline_passthrough_terminal() {
    assert!(is_psreadline_passthrough_terminal_class(
        "CASCADIA_HOSTING_WINDOW_CLASS",
        "Windows.UI.Input.InputSite.WindowClass"
    ));
    assert!(!is_psreadline_passthrough_terminal_class(
        "ApplicationFrameWindow",
        "Windows.UI.Input.InputSite.WindowClass"
    ));
    assert!(!is_psreadline_passthrough_terminal_class(
        "ConsoleWindowClass",
        "ConsoleWindowClass"
    ));
}

#[test]
fn classic_console_does_not_need_conservative_suppression() {
    assert!(!terminal_class_needs_conservative_suppression(
        "ConsoleWindowClass",
        "ConsoleWindowClass"
    ));
}

#[test]
fn windows_terminal_needs_conservative_suppression() {
    assert!(terminal_class_needs_conservative_suppression(
        "CASCADIA_HOSTING_WINDOW_CLASS",
        "Windows.UI.Input.InputSite.WindowClass"
    ));
}

#[test]
fn cmd_terminal_title_is_detected() {
    assert!(is_cmd_terminal_title("C:\\WINDOWS\\system32\\cmd.exe"));
    assert!(!is_cmd_terminal_title("PowerShell 7 (x64)"));
}

#[test]
fn ssh_terminal_title_is_detected_without_matching_powershell() {
    assert!(is_ssh_terminal_title("vpnuser"));
    assert!(is_ssh_terminal_title("root@host"));
    assert!(is_ssh_terminal_title("ssh user@host"));
    assert!(is_ssh_remote_adapter_title(
        "stepler-remote-ready vpnuser@host:~"
    ));
    assert!(!is_ssh_terminal_title("Windows PowerShell"));
    assert!(!is_ssh_terminal_title("PowerShell"));
    assert!(!is_ssh_terminal_title("PowerShell 7 (x64)"));
    assert!(!is_ssh_terminal_title("C:\\WINDOWS\\system32\\cmd.exe"));
}

#[test]
fn terminal_passthrough_keeps_only_local_powershell_forwardable() {
    assert_eq!(
        terminal_passthrough_for_window(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
            "Windows PowerShell"
        ),
        TerminalPassthrough::PsReadLine
    );
    assert_eq!(
        terminal_passthrough_for_window(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
            "PowerShell"
        ),
        TerminalPassthrough::PsReadLine
    );
    assert_eq!(
        terminal_passthrough_for_window(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
            "stepler-remote-ready vpnuser@host:~"
        ),
        TerminalPassthrough::SshRemote
    );
    assert_eq!(
        terminal_passthrough_for_window(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
            "vpnuser"
        ),
        TerminalPassthrough::Ssh
    );
    assert_eq!(
        terminal_passthrough_for_window(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
            "PowerShell ssh user@host"
        ),
        TerminalPassthrough::Ssh
    );
    assert_eq!(
        terminal_passthrough_for_window(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
            "Qwen CLI"
        ),
        TerminalPassthrough::TerminalApp
    );
    assert_eq!(
        terminal_passthrough_for_window(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
            "stepler-terminal-app qwen"
        ),
        TerminalPassthrough::TerminalApp
    );
    assert_eq!(
        terminal_passthrough_for_window(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
            ""
        ),
        TerminalPassthrough::UnknownTerminal
    );
    assert_eq!(
        terminal_passthrough_for_window(
            "Qt51518QWindowIcon",
            "Qt51518QWindowIcon",
            "\u{200e}Contact Name @ \u{200e}username"
        ),
        TerminalPassthrough::None
    );
}

#[test]
fn codex_embedded_terminal_host_titles_are_allowlisted() {
    assert!(is_codex_embedded_terminal_host_title("Codex"));
    assert!(is_codex_embedded_terminal_host_title("ChatGPT"));
    assert!(!is_codex_embedded_terminal_host_title(
        "ChatGPT - Google Chrome"
    ));
    assert!(!is_codex_embedded_terminal_host_title("Windows PowerShell"));
}

#[test]
fn context_capabilities_carry_method_binding() {
    let capabilities = Capabilities {
        can_replace_directly: true,
        can_read_selection: true,
        can_read_caret: true,
        method_binding: Some(MethodBinding::new(
            MethodId::Win32EditMessages,
            vec![MethodId::Win32EditMessages],
        )),
    };

    let binding = capabilities.method_binding.unwrap();
    assert_eq!(binding.context_method, MethodId::Win32EditMessages);
    assert_eq!(binding.replace_methods, vec![MethodId::Win32EditMessages]);
}

#[test]
fn context_replacement_method_uses_first_bound_replace_method() {
    let context = TextContext {
        app_id: String::from("ConsoleWindowClass"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("terminal-console:hwnd:1"),
        text_snapshot: String::from("пше"),
        caret_range: TextRange::caret("пше".len()),
        selection_range: None,
        capabilities: Capabilities {
            can_replace_directly: false,
            can_read_selection: false,
            can_read_caret: false,
            method_binding: Some(MethodBinding::new(
                MethodId::ConsoleBuffer,
                vec![MethodId::ConsoleBuffer],
            )),
        },
    };

    assert_eq!(
        context_replacement_method(&context),
        Some(MethodId::ConsoleBuffer)
    );
}

#[cfg(windows)]
#[test]
fn apply_replacement_requires_method_binding() {
    let context = TextContext {
        app_id: String::from("Notepad"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
        text_snapshot: String::from("k.,jdm"),
        caret_range: TextRange::caret("k.,jdm".len()),
        selection_range: None,
        capabilities: Capabilities {
            can_replace_directly: true,
            can_read_selection: true,
            can_read_caret: true,
            method_binding: None,
        },
    };
    let plan = ReplacementPlan {
        range: TextRange::new(0, "k.,jdm".len()),
        replacement_text: String::from("любовь"),
        reason: String::from("test"),
        confidence: 1.0,
        expected_before_text: String::from("k.,jdm"),
    };

    let error = apply_replacement(&context, &plan).unwrap_err();

    assert_eq!(
        error,
        PlatformError::ReplacementUnavailableReason(String::from("missing_method_binding"))
    );
}

#[cfg(windows)]
#[test]
fn apply_replacement_does_not_guess_terminal_adapter_without_binding() {
    let context = TextContext {
        app_id: String::from("ConsoleWindowClass"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("terminal-console:hwnd:1"),
        text_snapshot: String::from("пше"),
        caret_range: TextRange::caret("пше".len()),
        selection_range: None,
        capabilities: Capabilities {
            can_replace_directly: false,
            can_read_selection: false,
            can_read_caret: false,
            method_binding: None,
        },
    };
    let plan = ReplacementPlan {
        range: TextRange::new(0, "пше".len()),
        replacement_text: String::from("git"),
        reason: String::from("test"),
        confidence: 1.0,
        expected_before_text: String::from("пше"),
    };

    let error = apply_replacement(&context, &plan).unwrap_err();

    assert_eq!(
        error,
        PlatformError::ReplacementUnavailableReason(String::from("missing_method_binding"))
    );
}

#[test]
fn production_like_test_contexts_have_method_binding() {
    let contexts = [
        TextContext {
            app_id: String::from("Notepad"),
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
            text_snapshot: String::from("k.,jdm"),
            caret_range: TextRange::caret("k.,jdm".len()),
            selection_range: None,
            capabilities: Capabilities {
                can_replace_directly: true,
                can_read_selection: true,
                can_read_caret: true,
                method_binding: Some(MethodBinding::new(
                    MethodId::Win32EditMessages,
                    vec![MethodId::Win32EditMessages],
                )),
            },
        },
        TextContext {
            app_id: String::from("ConsoleWindowClass"),
            window_id: String::from("hwnd:1"),
            control_id: String::from("terminal-console:hwnd:1"),
            text_snapshot: String::from("пше"),
            caret_range: TextRange::caret("пше".len()),
            selection_range: None,
            capabilities: Capabilities {
                can_replace_directly: false,
                can_read_selection: false,
                can_read_caret: false,
                method_binding: Some(MethodBinding::new(
                    MethodId::ConsoleBuffer,
                    vec![MethodId::ConsoleBuffer],
                )),
            },
        },
        TextContext {
            app_id: String::from("Chrome_WidgetWin_1/Chrome_RenderWidgetHostHWND"),
            window_id: String::from("hwnd:1"),
            control_id: String::from("web-keyboard-selection:hwnd:2"),
            text_snapshot: String::from("ghbdtn"),
            caret_range: TextRange::caret("ghbdtn".len()),
            selection_range: None,
            capabilities: Capabilities {
                can_replace_directly: false,
                can_read_selection: true,
                can_read_caret: true,
                method_binding: Some(MethodBinding::new(
                    MethodId::WebKeyboardSelection,
                    vec![MethodId::WebKeyboardSelection],
                )),
            },
        },
    ];

    for context in contexts {
        assert!(
            context.capabilities.method_binding.is_some(),
            "{} should carry method binding",
            context.control_id
        );
    }
}

#[cfg(windows)]
#[test]
fn win32_edit_method_probes_supported_edit_controls() {
    let target = ForegroundTarget {
        app_class: String::from("Notepad"),
        focused_class: String::from("Edit"),
        title: String::new(),
        process_name: None,
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = Win32EditMessagesMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::Win32EditMessages);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
}

#[cfg(windows)]
#[test]
fn win32_edit_method_probes_outlook_richedit_controls() {
    let target = ForegroundTarget {
        app_class: String::from("rctrl_renwnd32"),
        focused_class: String::from("RICHEDIT60W"),
        title: String::from("Inbox - Outlook"),
        process_name: Some(String::from("OUTLOOK")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert_eq!(
        Win32EditMessagesMethod
            .probe(&target)
            .map(|probe| probe.method_id),
        Some(MethodId::Win32EditMessages)
    );
}

#[cfg(windows)]
#[test]
fn win32_edit_method_still_probes_outlook_plain_edit_controls() {
    let target = ForegroundTarget {
        app_class: String::from("rctrl_renwnd32"),
        focused_class: String::from("Edit"),
        title: String::from("Inbox - Outlook"),
        process_name: Some(String::from("OUTLOOK")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert_eq!(
        Win32EditMessagesMethod
            .probe(&target)
            .map(|probe| probe.method_id),
        Some(MethodId::Win32EditMessages)
    );
}

#[test]
fn win32_edit_adjusts_caret_after_replacement_before_caret() {
    let text = "house вальс gjkt long привет мир";
    let expected = "gjkt";
    let start = text.find(expected).unwrap();
    let end = start + expected.len();
    let context = TextContext::new(text).with_caret(TextRange::caret(text.len()));
    let plan = ReplacementPlan {
        range: TextRange::new(start, end),
        replacement_text: String::from("поле"),
        reason: String::from("test"),
        confidence: 1.0,
        expected_before_text: String::from(expected),
    };

    let adjusted = win32_adjusted_caret_after_replacement(&context, &plan);

    assert_eq!(adjusted, "house вальс поле long привет мир".len());
}

#[cfg(windows)]
#[test]
fn console_buffer_method_probes_classic_console() {
    let target = ForegroundTarget {
        app_class: String::from("ConsoleWindowClass"),
        focused_class: String::from("ConsoleWindowClass"),
        title: String::new(),
        process_name: None,
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:1"),
    };

    let probe = ConsoleBufferMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::ConsoleBuffer);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
}

#[cfg(windows)]
#[test]
fn terminal_clipboard_method_probes_windows_terminal_as_risky() {
    let target = ForegroundTarget {
        app_class: String::from("CASCADIA_HOSTING_WINDOW_CLASS"),
        focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
        title: String::new(),
        process_name: None,
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = TerminalClipboardShortcutMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::TerminalClipboardShortcut);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Risky);
    assert!(probe.requires_clipboard);
}

#[cfg(windows)]
#[test]
fn ssh_terminal_method_probes_ssh_title_as_unsupported() {
    let target = ForegroundTarget {
        app_class: String::from("CASCADIA_HOSTING_WINDOW_CLASS"),
        focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
        title: String::from("vpnuser"),
        process_name: None,
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = SshTerminalMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::SshTerminal);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Unsupported);
    assert!(!probe.requires_clipboard);
}

#[cfg(windows)]
#[test]
fn clipboard_selection_method_probes_unknown_controls_as_risky() {
    let target = ForegroundTarget {
        app_class: String::from("CustomAppWindow"),
        focused_class: String::from("CustomTextSurface"),
        title: String::new(),
        process_name: None,
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = ClipboardSelectionMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::ClipboardSelection);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Risky);
    assert!(probe.requires_clipboard);
}

#[cfg(windows)]
#[test]
fn send_input_method_probes_unknown_controls_as_risky_without_clipboard() {
    let target = ForegroundTarget {
        app_class: String::from("CustomAppWindow"),
        focused_class: String::from("CustomTextSurface"),
        title: String::new(),
        process_name: None,
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = SendInputMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::SendInput);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Risky);
    assert!(!probe.requires_clipboard);
}

#[cfg(windows)]
#[test]
fn uia_text_method_probes_unknown_non_special_controls() {
    let target = ForegroundTarget {
        app_class: String::from("ApplicationFrameWindow"),
        focused_class: String::from("Windows.UI.Core.CoreWindow"),
        title: String::from("Settings"),
        process_name: None,
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = UiAutomationTextMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::UiAutomationText);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
}

#[cfg(windows)]
#[test]
fn uia_editable_text_method_probes_browser_like_controls_as_strict_fallback() {
    let target = ForegroundTarget {
        app_class: String::from("Chrome_WidgetWin_1"),
        focused_class: String::from("Chrome_RenderWidgetHostHWND"),
        title: String::from("Confluence"),
        process_name: Some(String::from("chrome")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = UiAutomationEditableTextMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::UiAutomationEditableText);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
}

#[cfg(windows)]
#[test]
fn uia_document_text_method_probes_browser_like_controls() {
    let target = ForegroundTarget {
        app_class: String::from("MozillaWindowClass"),
        focused_class: String::from("MozillaWindowClass"),
        title: String::from("Confluence"),
        process_name: Some(String::from("firefox")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = UiAutomationDocumentTextMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::UiAutomationDocumentText);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
    assert!(!probe.requires_clipboard);
}

#[cfg(windows)]
#[test]
fn web_keyboard_selection_method_probes_browser_like_controls() {
    let target = ForegroundTarget {
        app_class: String::from("MozillaWindowClass"),
        focused_class: String::from("MozillaWindowClass"),
        title: String::from("Confluence"),
        process_name: Some(String::from("firefox")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = WebKeyboardSelectionMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::WebKeyboardSelection);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
    assert!(probe.requires_clipboard);
}

#[cfg(windows)]
#[test]
fn sticky_notes_runtime_stack_keeps_web_keyboard_as_late_fallback() {
    let target = ForegroundTarget {
        app_class: String::from("ApplicationFrameWindow"),
        focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
        title: String::from("Sticky Notes"),
        process_name: Some(String::from("Microsoft.Notes")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let method_ids = windows_runtime_probe_methods(&target);

    assert!(method_ids.contains(&MethodId::UiAutomationDocumentText));
    assert!(method_ids.contains(&MethodId::WebKeyboardSelection));
    assert!(method_ids.contains(&MethodId::UiAutomationEditableText));
    assert_eq!(method_ids[0], MethodId::UiAutomationDocumentText);
    assert_eq!(method_ids[1], MethodId::WebKeyboardSelection);
}

#[cfg(windows)]
#[test]
fn sticky_notes_probe_stack_is_not_terminal() {
    let target = ForegroundTarget {
        app_class: String::from("ApplicationFrameWindow"),
        focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
        title: String::from("Sticky Notes"),
        process_name: Some(String::from("Microsoft.Notes")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let method_ids = windows_method_probes(&target)
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();

    assert!(method_ids.contains(&MethodId::UiAutomationDocumentText));
    assert!(method_ids.contains(&MethodId::WebKeyboardSelection));
    assert!(!method_ids.contains(&MethodId::TerminalClipboardShortcut));
    assert!(!method_ids.contains(&MethodId::XtermKeyboardSelection));
}

#[cfg(windows)]
#[test]
fn fast_web_keyboard_target_uses_probe_plan_runtime_stack() {
    let target = ForegroundTarget {
        app_class: String::from("Chrome_WidgetWin_1"),
        focused_class: String::from("Chrome_WidgetWin_1"),
        title: String::from("Codex"),
        process_name: Some(String::from("Codex")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probes = windows_method_probes(&target);
    let method_ids = probes
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();
    let plan_method_ids = windows_probe_plan_methods(&target);
    let runtime_method_ids = windows_runtime_probe_methods(&target);

    assert_eq!(
        plan_method_ids,
        vec![
            MethodId::WebKeyboardSelection,
            MethodId::UiAutomationEditableText
        ]
    );
    assert_eq!(runtime_method_ids, plan_method_ids);
    assert_eq!(method_ids, runtime_method_ids);
    assert_eq!(method_ids.first(), Some(&MethodId::WebKeyboardSelection));
    assert!(method_ids.contains(&MethodId::UiAutomationEditableText));
    assert!(!method_ids.contains(&MethodId::UiAutomationDocumentText));
    assert!(!method_ids.contains(&MethodId::UiAutomationText));
}

#[cfg(windows)]
#[test]
fn rocket_chat_search_runtime_stack_prefers_uia_editable_before_keyboard() {
    let target = ForegroundTarget {
        app_class: String::from("Chrome_WidgetWin_1"),
        focused_class: String::from("Chrome_WidgetWin_1"),
        title: String::from("No unread messages"),
        process_name: Some(String::from("Rocket.Chat")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let method_ids = windows_method_probes(&target)
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();

    assert_eq!(
        method_ids,
        vec![
            MethodId::UiAutomationEditableText,
            MethodId::WebKeyboardSelection
        ]
    );
}

#[cfg(windows)]
#[test]
fn outlook_runtime_stacks_do_not_include_generic_fallbacks() {
    let cases = [
        (
            ForegroundTarget {
                app_class: String::from("rctrl_renwnd32"),
                focused_class: String::from("_WwG"),
                title: String::from("Untitled - Message"),
                process_name: Some(String::from("OUTLOOK")),
                window_id: String::from("hwnd:1"),
                control_id: String::from("hwnd:2"),
            },
            vec![MethodId::WordCom],
        ),
        (
            ForegroundTarget {
                app_class: String::from("rctrl_renwnd32"),
                focused_class: String::from("Edit"),
                title: String::from("Outlook"),
                process_name: Some(String::from("OUTLOOK")),
                window_id: String::from("hwnd:1"),
                control_id: String::from("hwnd:2"),
            },
            vec![MethodId::Win32EditMessages],
        ),
        (
            ForegroundTarget {
                app_class: String::from("rctrl_renwnd32"),
                focused_class: String::from("SUPERGRID"),
                title: String::from("Inbox - Zimbra - Alexey Andreev - Outlook"),
                process_name: Some(String::from("OUTLOOK")),
                window_id: String::from("hwnd:1"),
                control_id: String::from("hwnd:2"),
            },
            vec![MethodId::Win32EditMessages, MethodId::WordCom],
        ),
    ];

    for (target, expected) in cases {
        let method_ids = windows_runtime_probe_methods(&target);

        assert_eq!(method_ids, expected);
        assert!(!method_ids.contains(&MethodId::UiAutomationEditableText));
        assert!(!method_ids.contains(&MethodId::UiAutomationDocumentText));
        assert!(!method_ids.contains(&MethodId::UiAutomationText));
        assert!(!method_ids.contains(&MethodId::TerminalClipboardShortcut));
        assert!(!method_ids.contains(&MethodId::ClipboardSelection));
        assert!(!method_ids.contains(&MethodId::SendInput));
    }
}

#[cfg(windows)]
#[test]
fn qwen_terminal_probe_stack_is_xterm_only() {
    let target = ForegroundTarget {
        app_class: String::from("CASCADIA_HOSTING_WINDOW_CLASS"),
        focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
        title: String::from("stepler-terminal-app qwen"),
        process_name: Some(String::from("WindowsTerminal")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let method_ids = windows_method_probes(&target)
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();

    assert_eq!(method_ids, vec![MethodId::XtermKeyboardSelection]);
    assert!(!method_ids.contains(&MethodId::TerminalClipboardShortcut));
}

#[cfg(windows)]
#[test]
fn web_keyboard_fast_context_is_line_compatible() {
    assert!(web_keyboard_fast_context(
        "web-keyboard-fast-selection:hwnd:1"
    ));
    assert!(web_keyboard_fast_context(
        "web-keyboard-fast-line-selection:hwnd:1"
    ));
    assert!(is_web_keyboard_line_context(
        "web-keyboard-fast-line-selection:hwnd:1"
    ));
    assert!(!web_keyboard_fast_context(
        "web-keyboard-line-selection:hwnd:1"
    ));
    assert!(web_keyboard_rocket_fast_context(
        "web-keyboard-rocket-fast-selection:hwnd:1"
    ));
    assert!(web_keyboard_fast_context(
        "web-keyboard-rocket-fast-line-selection:hwnd:1"
    ));
    assert!(is_web_keyboard_line_context(
        "web-keyboard-rocket-fast-line-selection:hwnd:1"
    ));
    assert!(web_keyboard_rocket_active_line_context(
        "web-keyboard-rocket-active-line-selection:hwnd:1"
    ));
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_context_uses_dedicated_apply_path() {
    let control_id = "web-keyboard-captured-left-selection:hwnd:1";

    assert!(web_keyboard_captured_left_context(control_id));
    assert!(!web_keyboard_fast_context(control_id));
    assert!(!is_web_keyboard_line_context(control_id));
    assert!(!web_keyboard_rocket_fast_context(control_id));
    assert!(!web_keyboard_rocket_active_line_context(control_id));
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_title_policy_blocks_confluence_wiki() {
    assert!(!web_keyboard_allows_captured_left_for_title(
        "Security features - Chips - GS-Labs Wiki — Mozilla Firefox"
    ));
    assert!(!web_keyboard_allows_fast_line_apply_for_title(
        "Security features - Chips - GS-Labs Wiki — Mozilla Firefox"
    ));
    assert!(!web_keyboard_allows_relaxed_line_preflight_for_title(
        "Security features - Chips - GS-Labs Wiki — Mozilla Firefox"
    ));
    assert!(!web_keyboard_allows_captured_left_for_title(
        "Edit page - Confluence - Mozilla Firefox"
    ));
    assert!(!web_keyboard_allows_fast_line_apply_for_title(
        "Edit page - Confluence - Mozilla Firefox"
    ));
    assert!(!web_keyboard_allows_relaxed_line_preflight_for_title(
        "Edit page - Confluence - Mozilla Firefox"
    ));
    assert!(web_keyboard_allows_captured_left_for_title("Codex"));
    assert!(web_keyboard_allows_fast_line_apply_for_title("Codex"));
    assert!(web_keyboard_allows_relaxed_line_preflight_for_title(
        "Codex"
    ));
    assert!(web_keyboard_allows_captured_left_for_title(
        "ABC-123 - Jira - Google Chrome"
    ));
    assert!(web_keyboard_allows_fast_line_apply_for_title(
        "ABC-123 - Jira - Google Chrome"
    ));
    assert!(web_keyboard_allows_relaxed_line_preflight_for_title(
        "ABC-123 - Jira - Google Chrome"
    ));
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_text_policy_blocks_multiline_browser_selection() {
    assert!(web_keyboard_allows_captured_left_text_for_surface(
        "Chrome_WidgetWin_1",
        "Chrome_WidgetWin_1",
        "3 ,thb dct"
    ));
    assert!(!web_keyboard_allows_captured_left_text_for_surface(
        "Chrome_WidgetWin_1",
        "Chrome_WidgetWin_1",
        "v2\n3 ,thb dct"
    ));
    assert!(web_keyboard_allows_captured_left_text_for_surface(
        "ApplicationFrameWindow",
        "Windows.UI.Input.InputSite.WindowClass",
        "Когда pfdbcytn cyjdf\r\nbcgjkmpeq outlookhaging.md"
    ));
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_apply_rejects_multiline_browser_selection() {
    let context = TextContext {
        app_id: String::from("Chrome_WidgetWin_1/Chrome_WidgetWin_1"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("web-keyboard-captured-left-selection:hwnd:2"),
        text_snapshot: String::from("3 ,thb dct"),
        caret_range: TextRange::caret("3 ,thb dct".len()),
        selection_range: None,
        capabilities: Capabilities::default(),
    };

    assert!(!web_keyboard_allows_captured_left_apply_selection(
        &context,
        "v2\n3 ,thb dct"
    ));

    let sticky_context = TextContext {
        app_id: String::from("ApplicationFrameWindow/Windows.UI.Input.InputSite.WindowClass"),
        ..context
    };
    assert!(web_keyboard_allows_captured_left_apply_selection(
        &sticky_context,
        "Когда pfdbcytn cyjdf\r\nbcgjkmpeq outlookhaging.md"
    ));
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_apply_allows_wrapped_list_tail_selection() {
    let context = TextContext {
        app_id: String::from("Chrome_WidgetWin_1/Chrome_WidgetWin_1"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("web-keyboard-captured-left-selection:hwnd:2"),
        text_snapshot: String::from("1. ,eltv pfgecrfnm d "),
        caret_range: TextRange::caret("1. ,eltv pfgecrfnm d ".len()),
        selection_range: None,
        capabilities: Capabilities::default(),
    };
    let selected_text = "1. ?\n   ,eltv pfgecrfnm d ";

    assert!(web_keyboard_allows_captured_left_apply_selection(
        &context,
        selected_text
    ));

    let plan =
        stepler_core::build_replacement_plan(&context, stepler_core::CorrectionMode::ScrollLock)
            .unwrap();
    let (replacement_text, actual_before_text) =
        web_keyboard_captured_left_replacement_text(&context, &plan, selected_text).unwrap();

    assert_eq!(actual_before_text, ",eltv pfgecrfnm d");
    assert_eq!(replacement_text, "будем запускать в ");
}

#[cfg(windows)]
#[test]
fn web_keyboard_precise_range_apply_is_confluence_line_only() {
    assert!(web_keyboard_uses_precise_range_apply(
        "Security features - Chips - GS-Labs Wiki — Mozilla Firefox",
        "web-keyboard-fast-line-selection:hwnd:1",
        true
    ));
    assert!(!web_keyboard_uses_precise_range_apply(
        "Security features - Chips - GS-Labs Wiki — Mozilla Firefox",
        "web-keyboard-fast-line-selection:hwnd:1",
        false
    ));
    assert!(!web_keyboard_uses_precise_range_apply(
        "Security features - Chips - GS-Labs Wiki — Mozilla Firefox",
        "web-keyboard-selection-selected:hwnd:1",
        true
    ));
    assert!(!web_keyboard_uses_precise_range_apply(
        "ABC-123 - Jira - Google Chrome",
        "web-keyboard-fast-line-selection:hwnd:1",
        true
    ));
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_context_trims_trailing_line_breaks_before_planning() {
    let context = web_keyboard_context(
        "ApplicationFrameWindow",
        "Windows.UI.Input.InputSite.WindowClass",
        0x1,
        0x2,
        "web-keyboard-captured-left-selection",
        String::from("\r\nbcgjkmpeq outlookhaging.md\r\n"),
        false,
    );

    assert_eq!(context.text_snapshot, "\r\nbcgjkmpeq outlookhaging.md");
    assert_eq!(
        context.caret_range,
        TextRange::caret(context.text_snapshot.len())
    );

    let plan = stepler_core::build_replacement_plan(&context, stepler_core::CorrectionMode::Pause)
        .unwrap();
    assert_eq!(plan.expected_before_text, "bcgjkmpeq");
    assert_eq!(plan.replacement_text, "используй");
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_replans_expanded_preflight_selection() {
    let context = TextContext {
        app_id: String::from("Chrome_WidgetWin_1/Chrome_WidgetWin_1"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("web-keyboard-captured-left-selection:hwnd:2"),
        text_snapshot: String::from("jnftn"),
        caret_range: TextRange::caret("jnftn".len()),
        selection_range: None,
        capabilities: Capabilities::default(),
    };
    let original_plan =
        stepler_core::build_replacement_plan(&context, stepler_core::CorrectionMode::ScrollLock)
            .unwrap();

    assert_eq!(original_plan.expected_before_text, "jnftn");
    assert_eq!(original_plan.replacement_text, "отает");

    let (replacement_text, actual_before_text) = web_keyboard_captured_left_replacement_text(
        &context,
        &original_plan,
        "ну теперь то hf,jnftn",
    )
    .unwrap();

    assert_eq!(actual_before_text, "hf,jnftn");
    assert_eq!(replacement_text, "ну теперь то работает");
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_scrolllock_rejects_multiline_preflight_prefix() {
    let context = TextContext {
        app_id: String::from("MozillaWindowClass/MozillaWindowClass"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("web-keyboard-captured-left-selection:hwnd:2"),
        text_snapshot: String::from("jnftn"),
        caret_range: TextRange::caret("jnftn".len()),
        selection_range: None,
        capabilities: Capabilities::default(),
    };
    let plan =
        stepler_core::build_replacement_plan(&context, stepler_core::CorrectionMode::ScrollLock)
            .unwrap();

    let error = web_keyboard_captured_left_replacement_text(
        &context,
        &plan,
        "Confluence table above\r\nhf,jnftn",
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PlatformError::ReplacementUnavailableReason(reason)
            if reason.starts_with("web_keyboard_captured_left_preflight multiline_prefix")
    ));
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_retries_short_suffix_selection() {
    let context = TextContext {
        app_id: String::from("MozillaWindowClass/MozillaWindowClass"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("web-keyboard-captured-left-selection:hwnd:2"),
        text_snapshot: String::from("z cjplfk"),
        caret_range: TextRange::caret("z cjplfk".len()),
        selection_range: None,
        capabilities: Capabilities::default(),
    };

    assert!(web_keyboard_captured_left_should_retry_selection(
        &context, "cjplfk"
    ));
    assert!(!web_keyboard_captured_left_should_retry_selection(
        &context, "z cjplfk"
    ));
    assert!(!web_keyboard_captured_left_should_retry_selection(
        &context, "abc"
    ));
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_pause_rejects_non_whitespace_preflight_prefix() {
    let context = TextContext {
        app_id: String::from("ApplicationFrameWindow/Windows.UI.Input.InputSite.WindowClass"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("web-keyboard-captured-left-selection:hwnd:2"),
        text_snapshot: String::from("\r\nbcgjkmpeq outlookhaging.md"),
        caret_range: TextRange::caret("\r\nbcgjkmpeq outlookhaging.md".len()),
        selection_range: None,
        capabilities: Capabilities::default(),
    };
    let plan = stepler_core::build_replacement_plan(&context, stepler_core::CorrectionMode::Pause)
        .unwrap();

    let error = web_keyboard_captured_left_replacement_text(
        &context,
        &plan,
        "Напиши мне, и лучше сразу пришли вывод:\r\nbcgjkmpeq outlookhaging.md",
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PlatformError::ReplacementUnavailableReason(reason)
            if reason.starts_with("web_keyboard_captured_left_preflight non_whitespace_prefix")
    ));
}

#[cfg(windows)]
#[test]
fn web_keyboard_captured_left_trims_trailing_line_breaks_for_sticky_notes() {
    let context = TextContext {
        app_id: String::from("ApplicationFrameWindow/Windows.UI.Input.InputSite.WindowClass"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("web-keyboard-captured-left-selection:hwnd:2"),
        text_snapshot: String::from(". \r\nbcgjkpeq outlookhaging.md\r\n"),
        caret_range: TextRange::caret(". \r\nbcgjkpeq outlookhaging.md\r\n".len()),
        selection_range: None,
        capabilities: Capabilities::default(),
    };
    let original_plan =
        stepler_core::build_replacement_plan(&context, stepler_core::CorrectionMode::Pause)
            .unwrap_err();
    assert_eq!(
        original_plan,
        stepler_core::CorrectionError::NoTextToReplace
    );

    let context_without_suffix = TextContext {
        text_snapshot: String::from(". \r\nbcgjkpeq outlookhaging.md"),
        caret_range: TextRange::caret(". \r\nbcgjkpeq outlookhaging.md".len()),
        ..context.clone()
    };
    let plan = stepler_core::build_replacement_plan(
        &context_without_suffix,
        stepler_core::CorrectionMode::Pause,
    )
    .unwrap();

    let (replacement_text, actual_before_text) = web_keyboard_captured_left_replacement_text(
        &context,
        &plan,
        ". \r\nbcgjkpeq outlookhaging.md\r\n",
    )
    .unwrap();

    assert_eq!(actual_before_text, "bcgjkpeq");
    assert_eq!(replacement_text, ". \r\nисползуй outlookhaging.md\r\n");
}

#[cfg(windows)]
#[test]
fn web_keyboard_sticky_line_replans_expanded_selection() {
    let context = TextContext {
        app_id: String::from("ApplicationFrameWindow/Windows.UI.Input.InputSite.WindowClass"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("web-keyboard-line-selection:hwnd:2"),
        text_snapshot: String::from(
            "Когда pfdbcytn cyjdf, главное не перезапускать Outlook сразу.  ",
        ),
        caret_range: TextRange::caret(
            "Когда pfdbcytn cyjdf, главное не перезапускать Outlook сразу.  ".len(),
        ),
        selection_range: None,
        capabilities: Capabilities::default(),
    };
    let plan =
        stepler_core::build_replacement_plan(&context, stepler_core::CorrectionMode::ScrollLock)
            .unwrap();

    let selected_text = concat!(
        "Предыдущая строка\r\n",
        "Когда pfdbcytn cyjdf, главное не перезапускать Outlook сразу.  "
    );
    let (replacement_text, actual_before_text) =
        web_keyboard_sticky_line_replacement_text(&context, &plan, selected_text).unwrap();

    assert_eq!(actual_before_text, "pfdbcytn cyjdf,");
    assert_eq!(
        replacement_text,
        concat!(
            "Предыдущая строка\r\n",
            "Когда зависнет сноваб главное не перезапускать Outlook сразу.  "
        )
    );
}

#[cfg(windows)]
#[test]
fn rocket_active_line_context_does_not_mark_technical_selection_as_user_selection() {
    let context = TextContext {
        app_id: String::from("Chrome_WidgetWin_1/Chrome_RenderWidgetHostHWND"),
        window_id: String::from("hwnd:1"),
        control_id: String::from("web-keyboard-rocket-active-line-selection:hwnd:2"),
        text_snapshot: String::from("hello ghbdtn"),
        caret_range: TextRange::caret("hello ghbdtn".len()),
        selection_range: None,
        capabilities: Capabilities::default(),
    };

    assert_eq!(context.selection_range, None);

    let plan = stepler_core::build_replacement_plan(&context, stepler_core::CorrectionMode::Pause)
        .unwrap();

    assert_eq!(
        plan.range,
        TextRange::new("hello ".len(), "hello ghbdtn".len())
    );
    assert_eq!(plan.expected_before_text, "ghbdtn");
    assert_eq!(plan.replacement_text, "привет");
}

#[cfg(windows)]
#[test]
fn web_keyboard_fast_profile_matches_checked_web_apps() {
    assert!(web_keyboard_fast_profile_title_matches(
        "[CTP-11796] GS-Labs JIRA - Mozilla Firefox"
    ));
    assert!(web_keyboard_fast_profile_title_matches(
        "CVE - Chips - GS-Labs Wiki - Google Chrome"
    ));
    assert!(web_keyboard_fast_profile_title_matches(
        "Нет непрочитанных сообщений"
    ));
    assert!(web_keyboard_rocket_fast_profile_title_matches(
        "Нет непрочитанных сообщений"
    ));
    assert!(web_keyboard_fast_profile_title_matches(
        "2 unread messages - general - GS.Chat"
    ));
    assert!(web_keyboard_fast_profile_title_matches("Codex"));
}

#[cfg(windows)]
#[test]
fn web_keyboard_timing_profiles_are_profile_specific() {
    let standard = web_keyboard_timing_profile(WebKeyboardProfile::Standard);
    let fast = web_keyboard_timing_profile(WebKeyboardProfile::Fast);
    let rocket = web_keyboard_timing_profile(WebKeyboardProfile::RocketSearch);

    assert_ne!(standard.selected_timeout, fast.selected_timeout);
    assert_eq!(fast, rocket);
    assert!(fast.selected_timeout < standard.selected_timeout);
    assert!(fast.clipboard_timeout < standard.clipboard_timeout);
}

#[cfg(windows)]
#[test]
fn web_keyboard_control_prefixes_follow_profile_only() {
    assert_eq!(
        web_keyboard_control_prefix("web-keyboard-selection", WebKeyboardProfile::Standard),
        "web-keyboard-selection"
    );
    assert_eq!(
        web_keyboard_control_prefix("web-keyboard-selection", WebKeyboardProfile::Fast),
        "web-keyboard-fast-selection"
    );
    assert_eq!(
        web_keyboard_control_prefix("web-keyboard-line-selection", WebKeyboardProfile::Fast),
        "web-keyboard-fast-line-selection"
    );
    assert_eq!(
        web_keyboard_control_prefix("web-keyboard-selection", WebKeyboardProfile::RocketSearch),
        "web-keyboard-rocket-fast-selection"
    );
    assert_eq!(
        web_keyboard_control_prefix(
            "web-keyboard-line-selection",
            WebKeyboardProfile::RocketSearch
        ),
        "web-keyboard-rocket-fast-line-selection"
    );
}

#[cfg(windows)]
#[test]
fn web_keyboard_technical_target_does_not_expand_unknown_runtime_stack() {
    let target = ForegroundTarget {
        app_class: String::from("SomeCustomWindow"),
        focused_class: String::from("SomeCustomControl"),
        title: String::from("Custom"),
        process_name: Some(String::from("custom")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert!(!is_web_keyboard_technical_target(&target));
    let method_ids = windows_method_probes(&target)
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();
    assert_eq!(
        method_ids,
        vec![
            MethodId::UiAutomationEditableText,
            MethodId::UiAutomationDocumentText,
            MethodId::UiAutomationText,
        ]
    );
    let plan = probe_plan_for(&target);
    assert!(plan
        .suppressed_methods
        .contains(&MethodId::ClipboardSelection));
    assert!(plan.suppressed_methods.contains(&MethodId::SendInput));
    assert!(plan
        .suppressed_methods
        .contains(&MethodId::WebKeyboardSelection));
}

#[cfg(windows)]
#[test]
fn excel_cell_editor_gets_only_the_explicit_keyboard_selection_stack() {
    let editor = ForegroundTarget {
        app_class: String::from("XLMAIN"),
        focused_class: String::from("EXCEL6"),
        title: String::from("Book1 - Excel"),
        process_name: Some(String::from("EXCEL")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert!(is_web_keyboard_technical_target(&editor));
    let editor_methods = windows_method_probes(&editor)
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();
    assert_eq!(editor_methods, vec![MethodId::WebKeyboardSelection]);

    let workbook = ForegroundTarget {
        focused_class: String::from("EXCEL7"),
        ..editor
    };
    assert!(!is_web_keyboard_technical_target(&workbook));
    assert!(!windows_method_probes(&workbook)
        .iter()
        .any(|probe| probe.method_id == MethodId::WebKeyboardSelection));
}

#[cfg(windows)]
#[test]
fn hotkey_failure_trace_summary_includes_probe_and_resolver_boundaries() {
    let target = ForegroundTarget {
        app_class: String::from("SomeCustomWindow"),
        focused_class: String::from("SomeCustomControl"),
        title: String::from("Custom"),
        process_name: Some(String::from("custom")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let summary = diagnostics::hotkey_failure_trace_summary_for_target(
        &target,
        stepler_core::CorrectionMode::Pause,
        "Platform(ReplacementUnavailable)",
    );

    assert!(summary.contains("surface=Unknown"));
    assert!(summary.contains("mode=Pause"));
    assert!(summary.contains("probe_plan=["));
    assert!(summary.contains("runtime=["));
    assert!(summary.contains("probes=["));
    assert!(summary.contains("probe_none=["));
    assert!(summary.contains("suppressed=["));
    assert!(summary.contains("policy_skipped=["));
    assert!(summary.contains("final=operation_failed:Platform(ReplacementUnavailable)"));
}

#[cfg(windows)]
#[test]
fn web_keyboard_rejects_browser_document_dump_as_field_text() {
    assert!(is_plausible_web_field_text("gjkt"));
    assert!(is_plausible_web_field_text("secure OTP"));
    assert!(!is_plausible_web_field_text(
        "2 unread messages - general\nSkip to main content\ngit"
    ));
    assert!(!is_plausible_web_field_text(&"a".repeat(513)));
}

#[cfg(windows)]
#[test]
fn shifted_web_selection_prefix_accepts_safe_editor_prefixes() {
    assert_eq!(
        shifted_web_selection_prefix(" ghbqltncz ", "ghbqltncz "),
        Some(" ")
    );
    assert_eq!(
        shifted_web_selection_prefix("1. ыуьпкфз", "ыуьпкфз"),
        Some("1. ")
    );
    assert_eq!(
        shifted_web_selection_prefix("  12. ыуьпкфз", "ыуьпкфз"),
        Some("  12. ")
    );
    assert_eq!(
        shifted_web_selection_prefix("xghbqltncz ", "ghbqltncz "),
        Some("x")
    );
    assert_eq!(
        shifted_web_selection_prefix(" ghbqltncz !", "ghbqltncz "),
        None
    );
    assert_eq!(
        shifted_web_selection_prefix("openVAS - ыуьпкфз", "ыуьпкфз"),
        Some("openVAS - ")
    );
    assert_eq!(
        shifted_web_selection_prefix("jo, Api rk.x ", "rk.x "),
        Some("jo, Api ")
    );
    assert_eq!(
        shifted_web_selection_prefix("строка выше\nыуьпкфз", "ыуьпкфз"),
        None
    );
}

#[cfg(windows)]
#[test]
fn fast_web_selection_prefix_preserves_safe_editor_prefixes() {
    assert_eq!(
        accepted_fast_web_selection_prefix(Some(" (c,jh "), "(c,jh "),
        Some(String::from(" "))
    );
    assert_eq!(
        accepted_fast_web_selection_prefix(Some("1. ыуьпкфз"), "ыуьпкфз"),
        Some(String::from("1. "))
    );
    assert_eq!(
        accepted_fast_web_selection_prefix(Some("jo, Api rk.x "), "rk.x "),
        Some(String::from("jo, Api "))
    );
    assert_eq!(
        accepted_fast_web_selection_prefix(Some("(c,jh "), "(c,jh "),
        Some(String::new())
    );
    assert_eq!(
        accepted_fast_web_selection_prefix(Some("x(c,jh "), "(c,jh "),
        Some(String::from("x"))
    );
}

#[cfg(windows)]
#[test]
fn yandex_chrome_widget_is_browser_like() {
    let target = ForegroundTarget {
        app_class: String::from("Chrome_Yandex_WidgetWin_1"),
        focused_class: String::from("Chrome_Yandex_WidgetWin_1"),
        title: String::from("OneDrive"),
        process_name: Some(String::from("browser")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert!(is_browser_like_target(&target));
    assert!(WebKeyboardSelectionMethod.probe(&target).is_some());
    let method_ids = windows_method_probes(&target)
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();
    assert!(!method_ids.contains(&MethodId::ClipboardSelection));
    assert!(!method_ids.contains(&MethodId::SendInput));
}

#[cfg(windows)]
#[test]
fn telegram_qt_window_uses_keyboard_selection_probe() {
    let target = ForegroundTarget {
        app_class: String::from("Qt51518QWindowIcon"),
        focused_class: String::from("Qt51518QWindowIcon"),
        title: String::from("Contact @ username"),
        process_name: Some(String::from("Telegram")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert!(is_telegram_target(&target));
    assert!(WebKeyboardSelectionMethod.probe(&target).is_some());
    let method_ids = windows_method_probes(&target)
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();
    assert!(!method_ids.contains(&MethodId::ClipboardSelection));
    assert!(!method_ids.contains(&MethodId::SendInput));
}

#[cfg(windows)]
#[test]
fn uia_text_method_does_not_probe_known_special_controls() {
    let edit = ForegroundTarget {
        app_class: String::from("Notepad"),
        focused_class: String::from("Edit"),
        title: String::new(),
        process_name: None,
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };
    let terminal = ForegroundTarget {
        app_class: String::from("CASCADIA_HOSTING_WINDOW_CLASS"),
        focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
        title: String::new(),
        process_name: None,
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert!(UiAutomationTextMethod.probe(&edit).is_none());
    assert!(UiAutomationTextMethod.probe(&terminal).is_none());
}

#[cfg(windows)]
#[test]
fn generic_risky_methods_do_not_probe_browser_like_controls() {
    let target = ForegroundTarget {
        app_class: String::from("Chrome_WidgetWin_1"),
        focused_class: String::from("Chrome_RenderWidgetHostHWND"),
        title: String::from("Confluence"),
        process_name: Some(String::from("chrome")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let method_ids = windows_method_probes(&target)
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();

    assert!(!method_ids.contains(&MethodId::UiAutomationText));
    assert!(!method_ids.contains(&MethodId::ClipboardSelection));
    assert!(!method_ids.contains(&MethodId::SendInput));
}

#[cfg(windows)]
#[test]
fn browser_like_document_text_selection_is_safe_but_caret_fallback_is_blocked() {
    let target = ForegroundTarget {
        app_class: String::from("Chrome_WidgetWin_1"),
        focused_class: String::from("Chrome_WidgetWin_1"),
        title: String::from("Browser-like editor"),
        process_name: Some(String::from("chrome")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert_eq!(
        UiAutomationDocumentTextMethod
            .probe(&target)
            .map(|probe| probe.method_id),
        Some(MethodId::UiAutomationDocumentText)
    );
    assert!(!allow_uia_document_caret_fallback(&target));
    let method_ids = windows_method_probes(&target)
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();
    assert!(!method_ids.contains(&MethodId::UiAutomationText));
    assert!(!method_ids.contains(&MethodId::ClipboardSelection));
    assert!(!method_ids.contains(&MethodId::SendInput));
}

#[cfg(windows)]
#[test]
fn sticky_notes_document_text_allows_caret_fallback() {
    let target = ForegroundTarget {
        app_class: String::from("ApplicationFrameWindow"),
        focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
        title: String::from("Sticky Notes"),
        process_name: Some(String::from("Microsoft.Notes")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert_eq!(
        UiAutomationDocumentTextMethod
            .probe(&target)
            .map(|probe| probe.method_id),
        Some(MethodId::UiAutomationDocumentText)
    );
    assert!(allow_uia_document_caret_fallback(&target));
}

#[cfg(windows)]
#[test]
fn word_com_method_probes_word_windows() {
    let target = ForegroundTarget {
        app_class: String::from("OpusApp"),
        focused_class: String::from("_WwG"),
        title: String::from("Document1 - Word"),
        process_name: Some(String::from("WINWORD")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = WordComMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::WordCom);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
    assert!(!probe.requires_clipboard);
}

#[cfg(windows)]
#[test]
fn word_com_method_probes_outlook_word_editor_windows() {
    let target = ForegroundTarget {
        app_class: String::from("rctrl_renwnd32"),
        focused_class: String::from("_WwG"),
        title: String::from("Untitled - Message"),
        process_name: Some(String::from("OUTLOOK")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    let probe = WordComMethod.probe(&target).unwrap();

    assert_eq!(probe.method_id, MethodId::WordCom);
    assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
    assert!(!probe.requires_clipboard);
}

#[cfg(windows)]
#[test]
fn word_com_method_does_not_probe_outlook_explorer_windows() {
    let target = ForegroundTarget {
        app_class: String::from("rctrl_renwnd32"),
        focused_class: String::from("SUPERGRID"),
        title: String::from("Inbox - Outlook"),
        process_name: Some(String::from("OUTLOOK")),
        window_id: String::from("hwnd:1"),
        control_id: String::from("hwnd:2"),
    };

    assert!(WordComMethod.probe(&target).is_none());
}

#[test]
fn word_com_control_id_carries_absolute_base() {
    assert_eq!(parse_word_com_base("word-com:42:hwnd:ABC"), Some(42));
    assert_eq!(
        parse_word_com_base("outlook-word-com:42:hwnd:ABC"),
        Some(42)
    );
    assert_eq!(parse_word_com_base("word-com:nope:hwnd:ABC"), None);
}

#[test]
fn utf16le_base64_round_trips_word_text() {
    let encoded = encode_utf16le_base64("любовь");

    assert_eq!(encoded, "OwROBDEEPgQyBEwE");
    assert_eq!(decode_utf16le_base64(&encoded).unwrap(), "любовь");
}

#[test]
fn unsupported_control_error_keeps_diagnostic_classes() {
    let error = PlatformError::UnsupportedControl {
        app_class: String::from("Chrome_WidgetWin_1"),
        focused_class: String::from("Chrome_RenderWidgetHostHWND"),
    };

    assert_eq!(
            format!("{error:?}"),
            "UnsupportedControl { app_class: \"Chrome_WidgetWin_1\", focused_class: \"Chrome_RenderWidgetHostHWND\" }"
        );
}

#[test]
fn slice_by_range_uses_byte_offsets_and_checks_boundaries() {
    let text = "привет мир";

    assert_eq!(
        slice_by_range(text, TextRange::new(0, "привет".len())),
        Some("привет")
    );
    assert_eq!(slice_by_range(text, TextRange::new(1, 3)), None);
}

#[test]
fn replace_range_text_preserves_terminal_prefix() {
    let text = "echo ghbdtn vbh";
    let start = text.find("ghbdtn").unwrap();
    let end = text.len();

    assert_eq!(
        replace_range_text(text, TextRange::new(start, end), "привет мир"),
        Some(String::from("echo привет мир"))
    );
}

#[test]
fn console_prompt_line_parser_extracts_input() {
    assert_eq!(
        console_input_from_prompt_line("PS C:\\Users\\User> пше      "),
        "пше"
    );
    assert_eq!(console_input_from_prompt_line("ghbdtn vbh"), "ghbdtn vbh");
}

#[test]
fn converts_offsets_between_utf16_and_utf8_boundaries() {
    let text = "a привет 🌍";
    let byte_offset = "a привет".len();
    let edit_offset = byte_offset_to_edit_offset(text, byte_offset).unwrap();

    assert_eq!(
        edit_offset_to_byte_offset(text, edit_offset),
        Some(byte_offset)
    );
    assert_eq!(edit_offset_to_byte_offset(text, 999), None);
    assert_eq!(byte_offset_to_edit_offset(text, 3), None);
}

#[test]
fn converts_edit_offsets_with_crlf_counted_as_one_position() {
    let text = "one\r\ntwo\r\nвальс поле long ghbdtn vbh";
    let ghbdtn_start = text.find("ghbdtn").unwrap();
    let ghbdtn_end = ghbdtn_start + "ghbdtn".len();

    let edit_start = byte_offset_to_edit_offset(text, ghbdtn_start).unwrap();
    let edit_end = byte_offset_to_edit_offset(text, ghbdtn_end).unwrap();

    assert!(edit_start < ghbdtn_start);
    assert!(edit_end < ghbdtn_end);
    assert_eq!(
        edit_offset_to_byte_offset(text, edit_start),
        Some(ghbdtn_start)
    );
    assert_eq!(edit_offset_to_byte_offset(text, edit_end), Some(ghbdtn_end));
    assert_eq!(
        byte_offset_to_edit_offset(text, text.find('\n').unwrap()),
        None
    );
}

#[test]
fn clipboard_wide_string_round_trips_with_nul_terminator() {
    let wide = string_to_null_terminated_utf16("тест");

    assert_eq!(wide.last(), Some(&0));
    assert_eq!(utf16_until_nul_to_string(&wide), "тест");
}

#[test]
fn global_memory_bytes_round_up_to_even_utf16_size() {
    let wide = string_to_null_terminated_utf16("ab");
    let bytes = utf16_to_le_bytes(&wide);

    assert_eq!(bytes.len(), wide.len() * 2);
    assert_eq!(le_bytes_to_utf16(&bytes), wide);
}

#[test]
fn clipboard_snapshot_can_hold_multiple_formats() {
    let snapshot = ClipboardSnapshot {
        text: Some(String::from("hello")),
        sequence_number: Some(42),
        formats: vec![
            ClipboardFormatSnapshot {
                format: 1,
                bytes: vec![1, 2, 3],
            },
            ClipboardFormatSnapshot {
                format: CF_UNICODETEXT,
                bytes: utf16_to_le_bytes(&string_to_null_terminated_utf16("hello")),
            },
        ],
    };

    assert_eq!(snapshot.formats.len(), 2);
    assert_eq!(snapshot.text.as_deref(), Some("hello"));
}

#[cfg(windows)]
#[test]
fn text_probe_restore_does_not_restore_non_text_clipboard_formats() {
    let snapshot = ClipboardSnapshot {
        text: None,
        sequence_number: Some(42),
        formats: vec![ClipboardFormatSnapshot {
            format: 8,
            bytes: vec![1, 2, 3, 4],
        }],
    };

    assert!(clipboard_snapshot_for_text_probe_restore(&snapshot).is_none());
}

#[cfg(windows)]
#[test]
fn text_probe_restore_rebuilds_unicode_text_only_snapshot() {
    let snapshot = ClipboardSnapshot {
        text: Some(String::from("hello")),
        sequence_number: Some(42),
        formats: vec![
            ClipboardFormatSnapshot {
                format: CF_UNICODETEXT,
                bytes: utf16_to_le_bytes(&string_to_null_terminated_utf16("hello")),
            },
            ClipboardFormatSnapshot {
                format: 8,
                bytes: vec![1, 2, 3, 4],
            },
        ],
    };

    let restore_snapshot = clipboard_snapshot_for_text_probe_restore(&snapshot).unwrap();

    assert_eq!(restore_snapshot.text.as_deref(), Some("hello"));
    assert_eq!(restore_snapshot.formats.len(), 1);
    assert_eq!(restore_snapshot.formats[0].format, CF_UNICODETEXT);
}

#[cfg(windows)]
#[test]
fn clipboard_hglobal_filter_skips_gdi_handle_formats() {
    assert!(!clipboard_format_uses_hglobal(CF_BITMAP));
    assert!(!clipboard_format_uses_hglobal(CF_ENHMETAFILE));
    assert!(clipboard_format_uses_hglobal(CF_UNICODETEXT));
}

#[test]
fn keyboard_control_action_message_ids_round_trip() {
    for action in [
        KeyboardControlAction::SwitchToRussian,
        KeyboardControlAction::SwitchToEnglish,
        KeyboardControlAction::SwitchToNext,
    ] {
        assert_eq!(
            KeyboardControlAction::from_message_id(action.message_id()),
            Some(action)
        );
    }
    assert_eq!(KeyboardControlAction::from_message_id(99), None);
}

#[test]
fn keyboard_control_state_switches_only_on_single_ctrl() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
    assert_eq!(
        state.handle_key(VK_LCONTROL, false, true),
        Some(KeyboardControlAction::SwitchToRussian)
    );

    assert_eq!(state.handle_key(VK_RCONTROL, true, false), None);
    assert_eq!(state.handle_key(0x43, true, false), None);
    assert_eq!(state.handle_key(VK_RCONTROL, false, true), None);
}

#[test]
fn keyboard_control_state_ignores_orphan_ctrl_up() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);
    assert_eq!(state.handle_key(VK_RCONTROL, false, true), None);
}

#[test]
fn keyboard_control_state_ignores_layout_controls_during_win_combo() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(state.handle_key(VK_LWIN, true, false), None);
    assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
    assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);
    assert_eq!(state.handle_key(VK_LWIN, false, true), None);
    assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
    assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);

    state.suspend_layout_controls_until = Some(Instant::now() - Duration::from_millis(1));
    assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
    assert_eq!(
        state.handle_key(VK_LCONTROL, false, true),
        Some(KeyboardControlAction::SwitchToRussian)
    );
}

#[test]
fn keyboard_control_state_recovers_from_missing_win_key_up() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(state.handle_key(VK_LWIN, true, false), None);
    state.suspend_layout_controls_until = Some(Instant::now() - Duration::from_millis(1));
    assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
    assert_eq!(
        state.handle_key(VK_LCONTROL, false, true),
        Some(KeyboardControlAction::SwitchToRussian)
    );
}

#[test]
fn keyboard_control_state_emits_correction_hotkey_once_until_key_up() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(
        state.handle_correction_hotkey(VK_PAUSE, true, false),
        Some(stepler_core::CorrectionMode::Pause)
    );
    assert_eq!(state.handle_correction_hotkey(VK_PAUSE, false, true), None);
    let mut state = KeyboardControlHookState::default();
    assert_eq!(
        state.handle_correction_hotkey(VK_CANCEL, true, false),
        Some(stepler_core::CorrectionMode::Pause)
    );
}

#[test]
fn keyboard_control_state_recovers_from_missing_pause_key_up() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(
        state.handle_correction_hotkey(VK_PAUSE, true, false),
        Some(stepler_core::CorrectionMode::Pause)
    );
    state.pause_down_at = Some(Instant::now() - Duration::from_millis(800));
    state.last_pause_at = Some(Instant::now() - Duration::from_millis(800));

    assert_eq!(
        state.handle_correction_hotkey(VK_PAUSE, true, false),
        Some(stepler_core::CorrectionMode::Pause)
    );
}

#[test]
fn keyboard_control_state_maps_ctrl_pause_to_scrolllock_mode() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
    assert_eq!(state.handle_correction_hotkey(VK_PAUSE, true, false), None);
    assert_eq!(state.handle_correction_hotkey(VK_PAUSE, false, true), None);
    assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);
    assert_eq!(
        state.take_pending_scroll_lock_if_released(),
        Some(stepler_core::CorrectionMode::ScrollLock)
    );

    let mut state = KeyboardControlHookState::default();
    assert_eq!(state.handle_key(VK_RCONTROL, true, false), None);
    assert_eq!(state.handle_correction_hotkey(VK_CANCEL, true, false), None);
    assert_eq!(state.handle_correction_hotkey(VK_CANCEL, false, true), None);
    assert_eq!(state.handle_key(VK_RCONTROL, false, true), None);
    assert_eq!(
        state.take_pending_scroll_lock_if_released(),
        Some(stepler_core::CorrectionMode::ScrollLock)
    );
}

#[test]
fn keyboard_control_state_marks_ctrl_pause_as_used_when_terminal_handles_it() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
    assert_eq!(
        state.handle_terminal_pause_key(VK_PAUSE, true, false),
        TerminalPauseHandling::TranslateToF14
    );
    assert_eq!(
        state.handle_terminal_pause_key(VK_PAUSE, false, true),
        TerminalPauseHandling::Suppress
    );
    assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);

    let mut state = KeyboardControlHookState::default();
    assert_eq!(state.handle_key(VK_RCONTROL, true, false), None);
    assert_eq!(
        state.handle_terminal_pause_key(VK_CANCEL, true, false),
        TerminalPauseHandling::TranslateToF14
    );
    assert_eq!(
        state.handle_terminal_pause_key(VK_CANCEL, false, true),
        TerminalPauseHandling::Suppress
    );
    assert_eq!(state.handle_key(VK_RCONTROL, false, true), None);
}

#[test]
fn keyboard_control_state_maps_classic_console_ctrl_pause_immediately() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
    assert_eq!(
        state.handle_classic_console_pause_key(VK_CANCEL, true, false),
        Some(stepler_core::CorrectionMode::ScrollLock)
    );
    assert_eq!(
        state.handle_classic_console_pause_key(VK_CANCEL, false, true),
        None
    );
    assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);
    assert_eq!(state.take_pending_scroll_lock_if_released(), None);
}

#[test]
fn keyboard_control_state_maps_classic_console_plain_pause_immediately() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(
        state.handle_classic_console_pause_key(VK_PAUSE, true, false),
        Some(stepler_core::CorrectionMode::Pause)
    );
    assert_eq!(
        state.handle_classic_console_pause_key(VK_PAUSE, false, true),
        None
    );
}

#[test]
fn keyboard_control_state_maps_plain_terminal_pause_to_psreadline_chord() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(
        state.handle_terminal_pause_key(VK_PAUSE, true, false),
        TerminalPauseHandling::TranslateToF13
    );
    assert_eq!(
        state.handle_terminal_pause_key(VK_PAUSE, false, true),
        TerminalPauseHandling::Suppress
    );
}

#[test]
fn keyboard_control_state_suppresses_scrolllock_companion_c_briefly() {
    let mut state = KeyboardControlHookState::default();

    assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
    assert_eq!(state.handle_correction_hotkey(VK_PAUSE, true, false), None);
    assert!(state.should_suppress_companion_key(VK_C));
    assert!(state.should_suppress_companion_key(VK_C));
    assert!(state.should_suppress_companion_key(VK_HOME));
    assert!(state.should_suppress_companion_key(VK_RIGHT));
    assert!(!state.should_suppress_companion_key(VK_LCONTROL));
}

#[cfg(windows)]
#[test]
fn injected_keyboard_events_are_not_interpreted_as_user_controls() {
    let event = KbdLlHookStruct {
        vk_code: VK_CONTROL,
        flags: LLKHF_INJECTED,
        ..KbdLlHookStruct::default()
    };

    assert!(should_ignore_keyboard_hook_event(event));
}

#[cfg(windows)]
#[test]
fn send_input_keyboard_struct_has_windows_x64_size() {
    assert_eq!(std::mem::size_of::<Input>(), 40);
}

#[cfg(windows)]
#[test]
fn send_input_uses_scan_codes_for_keyboard_events() {
    let input = Input::keyboard_scan_code(VK_C, false, 0);
    let keyboard = unsafe { input.input.ki };

    assert_eq!(keyboard.vk, 0);
    assert_ne!(keyboard.scan, 0);
    assert_eq!(keyboard.flags & KEYEVENTF_SCANCODE, KEYEVENTF_SCANCODE);
}

#[cfg(windows)]
#[test]
fn send_input_can_use_virtual_keys_for_terminal_shortcuts() {
    let input = Input::keyboard_virtual_key(VK_C, false, 0);
    let keyboard = unsafe { input.input.ki };

    assert_eq!(keyboard.vk, VK_C as u16);
    assert_eq!(keyboard.scan, 0);
    assert_eq!(keyboard.flags & KEYEVENTF_SCANCODE, 0);
}

#[cfg(windows)]
#[test]
fn send_input_can_emit_unicode_units() {
    let input = Input::keyboard_unicode('я' as u16, false);
    let keyboard = unsafe { input.input.ki };

    assert_eq!(keyboard.vk, 0);
    assert_eq!(keyboard.scan, 'я' as u16);
    assert_eq!(keyboard.flags & KEYEVENTF_UNICODE, KEYEVENTF_UNICODE);
}

#[test]
fn hotkeyhandler_markers_are_not_user_selection_text() {
    assert!(looks_like_hotkeyhandler_marker(
        "__HKH_SELECTED_MARKER_cc58bf97238446389051bab0525c89da__"
    ));
    assert!(looks_like_hotkeyhandler_marker(
        "__HKH_LEFT_TEXT_MARKER_cc58bf97238446389051bab0525c89da__"
    ));
    assert!(!looks_like_hotkeyhandler_marker("k.,jdm"));
}

#[cfg(windows)]
#[test]
fn send_input_marks_navigation_keys_as_extended() {
    for vk in [VK_HOME, VK_END, VK_INSERT] {
        let input = Input::keyboard_scan_code(vk, false, 0);
        let keyboard = unsafe { input.input.ki };

        assert_eq!(
            keyboard.flags & KEYEVENTF_EXTENDEDKEY,
            KEYEVENTF_EXTENDEDKEY,
            "vk=0x{vk:X} must not be sent as a numpad key"
        );
    }
}
