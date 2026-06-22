use stepler_core::{CorrectionMode, MethodId};
use stepler_platform::{
    adapter_contract, classify_surface, ForegroundTarget, MethodProbe, MethodResolver, SurfaceKind,
};

#[derive(Debug)]
struct ContractRow {
    name: String,
    target: ForegroundTarget,
    mode: CorrectionMode,
    probes: Vec<MethodId>,
    expected_surface: SurfaceKind,
    expected_context: MethodId,
    expected_replacement: MethodId,
    forbidden: Vec<MethodId>,
}

#[test]
fn resolver_contract_matrix_matches_verified_surfaces() {
    let rows = parse_fixture(include_str!("fixtures/resolver_contracts.tsv"));
    assert!(!rows.is_empty(), "resolver contract fixture is empty");

    let resolver = MethodResolver::default();
    for row in rows {
        let classification = classify_surface(&row.target);
        assert_eq!(
            classification.kind, row.expected_surface,
            "{}: surface evidence: {:?}",
            row.name, classification.evidence
        );

        let probes = row
            .probes
            .iter()
            .copied()
            .map(probe_for)
            .collect::<Vec<_>>();
        let decision = resolver
            .resolve_for_mode(&row.target, &probes, row.mode)
            .unwrap_or_else(|error| panic!("{}: resolver failed: {:?}", row.name, error));

        assert_eq!(
            decision.context_method, row.expected_context,
            "{}: unexpected context method",
            row.name
        );
        assert_eq!(
            decision.replacement_method, row.expected_replacement,
            "{}: unexpected replacement method",
            row.name
        );

        for forbidden in row.forbidden {
            let probes = [probe_for(forbidden)];
            assert!(
                resolver
                    .resolve_for_mode(&row.target, &probes, row.mode)
                    .is_err(),
                "{}: forbidden method {} was accepted",
                row.name,
                forbidden.as_str()
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
    assert_eq!(fields.len(), 11, "bad fixture row: {line}");

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
        mode: parse_mode(fields[5]),
        probes: parse_methods(fields[6]),
        expected_surface: parse_surface(fields[7]),
        expected_context: parse_method(fields[8]),
        expected_replacement: parse_method(fields[9]),
        forbidden: parse_methods(fields[10]),
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

fn parse_mode(value: &str) -> CorrectionMode {
    match value {
        "pause" => CorrectionMode::Pause,
        "scrolllock" => CorrectionMode::ScrollLock,
        _ => panic!("unknown correction mode: {value}"),
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
        "Unknown" => SurfaceKind::Unknown,
        _ => panic!("unknown surface kind: {value}"),
    }
}

fn probe_for(method: MethodId) -> MethodProbe {
    let contract = adapter_contract(method);
    if contract.risky {
        MethodProbe::risky(method, "fixture")
    } else {
        MethodProbe::safe(method, "fixture")
    }
}
