use stepler_core::MethodId;
use stepler_platform::{probe_plan_for, surface_policy_for, ForegroundTarget, SurfaceKind};

#[derive(Debug)]
struct ContractRow {
    name: String,
    target: ForegroundTarget,
    expected_surface: SurfaceKind,
    expected_probe_methods: Vec<MethodId>,
    expected_suppressed_methods: Vec<MethodId>,
    min_confidence: u8,
    fast_probe: bool,
}

#[test]
fn probe_contract_matrix_matches_verified_surfaces() {
    let rows = parse_fixture(include_str!("fixtures/probe_contracts.tsv"));
    assert!(!rows.is_empty(), "probe contract fixture is empty");

    for row in rows {
        let plan = probe_plan_for(&row.target);
        assert_eq!(
            plan.surface.kind, row.expected_surface,
            "{}: surface evidence: {:?}",
            row.name, plan.surface.evidence
        );
        assert!(
            plan.surface.confidence >= row.min_confidence,
            "{}: confidence {} below expected {}; evidence: {:?}",
            row.name,
            plan.surface.confidence,
            row.min_confidence,
            plan.surface.evidence
        );
        assert_eq!(
            plan.probe_methods, row.expected_probe_methods,
            "{}: probe methods",
            row.name
        );
        assert_eq!(plan.fast_probe, row.fast_probe, "{}: fast_probe", row.name);

        for method in &row.expected_suppressed_methods {
            assert!(
                plan.suppressed_methods.contains(method),
                "{}: expected suppressed method {} missing from {:?}",
                row.name,
                method.as_str(),
                plan.suppressed_methods
            );
        }

        let surface_policy = surface_policy_for(plan.surface.kind);
        for method in &plan.probe_methods {
            assert!(
                !surface_policy.forbidden_methods.contains(method),
                "{}: probe method {} is forbidden by surface policy",
                row.name,
                method.as_str()
            );
        }
    }
}

fn parse_fixture(input: &str) -> Vec<ContractRow> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(parse_row)
        .collect()
}

fn parse_row(line: &str) -> ContractRow {
    let fields = line.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 10, "bad fixture row: {line}");

    ContractRow {
        name: fields[0].to_owned(),
        target: ForegroundTarget {
            app_class: fields[1].to_owned(),
            focused_class: fields[2].to_owned(),
            process_name: optional_field(fields[3]),
            title: string_field(fields[4]),
            window_id: String::from("fixture-window"),
            control_id: String::from("fixture-control"),
        },
        expected_surface: parse_surface(fields[5]),
        expected_probe_methods: parse_methods(fields[6]),
        expected_suppressed_methods: parse_methods(fields[7]),
        min_confidence: fields[8]
            .parse()
            .unwrap_or_else(|error| panic!("bad min_confidence `{}`: {error}", fields[8])),
        fast_probe: parse_bool(fields[9]),
    }
}

fn optional_field(value: &str) -> Option<String> {
    (value != "-").then(|| value.to_owned())
}

fn string_field(value: &str) -> String {
    if value == "-" {
        String::new()
    } else {
        value.to_owned()
    }
}

fn parse_methods(value: &str) -> Vec<MethodId> {
    if value == "-" || value.is_empty() {
        Vec::new()
    } else {
        value.split(',').map(parse_method).collect()
    }
}

fn parse_bool(value: &str) -> bool {
    match value {
        "true" => true,
        "false" => false,
        _ => panic!("unknown bool: {value}"),
    }
}

fn parse_method(value: &str) -> MethodId {
    match value {
        "win32_edit_messages" => MethodId::Win32EditMessages,
        "terminal_clipboard_shortcut" => MethodId::TerminalClipboardShortcut,
        "ssh_terminal" => MethodId::SshTerminal,
        "console_buffer" => MethodId::ConsoleBuffer,
        "psreadline" => MethodId::PsReadLine,
        "word_com" => MethodId::WordCom,
        "uia_editable_text" => MethodId::UiAutomationEditableText,
        "uia_document_text" => MethodId::UiAutomationDocumentText,
        "uia_text" => MethodId::UiAutomationText,
        "xterm_keyboard_selection" => MethodId::XtermKeyboardSelection,
        "web_keyboard_selection" => MethodId::WebKeyboardSelection,
        "clipboard_selection" => MethodId::ClipboardSelection,
        "send_input" => MethodId::SendInput,
        _ => panic!("unknown method id: {value}"),
    }
}

fn parse_surface(value: &str) -> SurfaceKind {
    match value {
        "Win32Edit" => SurfaceKind::Win32Edit,
        "NotepadLike" => SurfaceKind::NotepadLike,
        "ClassicConsole" => SurfaceKind::ClassicConsole,
        "WindowsTerminalCmd" => SurfaceKind::WindowsTerminalCmd,
        "WindowsTerminalPowerShell" => SurfaceKind::WindowsTerminalPowerShell,
        "QwenTerminal" => SurfaceKind::QwenTerminal,
        "BrowserEditor" => SurfaceKind::BrowserEditor,
        "FastBrowserEditor" => SurfaceKind::FastBrowserEditor,
        "RocketChatEditor" => SurfaceKind::RocketChatEditor,
        "YandexBrowserEditor" => SurfaceKind::YandexBrowserEditor,
        "TelegramDesktop" => SurfaceKind::TelegramDesktop,
        "StickyNotes" => SurfaceKind::StickyNotes,
        "OutlookSearch" => SurfaceKind::OutlookSearch,
        "OutlookWordEditor" => SurfaceKind::OutlookWordEditor,
        "OutlookShell" => SurfaceKind::OutlookShell,
        "WordEditor" => SurfaceKind::WordEditor,
        "ExcelCellEditor" => SurfaceKind::ExcelCellEditor,
        "Unknown" => SurfaceKind::Unknown,
        _ => panic!("unknown surface kind: {value}"),
    }
}
