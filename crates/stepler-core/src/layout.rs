pub fn convert_layout_text(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    let russian_count = input
        .chars()
        .filter(|&c| russian_to_english(c).is_some())
        .count();
    let english_count = input
        .chars()
        .filter(|&c| english_to_russian(c).is_some())
        .count();
    let direction = if russian_count > english_count {
        LayoutDirection::RussianToEnglish
    } else {
        LayoutDirection::EnglishToRussian
    };

    convert_with_direction(input, direction)
}

pub fn convert_selected_text(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut token = String::new();

    for ch in input.chars() {
        if ch.is_whitespace() {
            if !token.is_empty() {
                result.push_str(&convert_token_by_script(&token));
                token.clear();
            }
            result.push(ch);
        } else {
            token.push(ch);
        }
    }

    if !token.is_empty() {
        result.push_str(&convert_token_by_script(&token));
    }

    result
}

#[derive(Clone, Copy)]
enum LayoutDirection {
    RussianToEnglish,
    EnglishToRussian,
}

fn convert_token_by_script(token: &str) -> String {
    let has_russian = token.chars().any(is_russian_letter);
    let has_english = token.chars().any(is_english_letter);

    match (has_russian, has_english) {
        (true, false) => convert_with_direction(token, LayoutDirection::RussianToEnglish),
        (false, true) => convert_with_direction(token, LayoutDirection::EnglishToRussian),
        _ => convert_layout_text(token),
    }
}

fn convert_with_direction(input: &str, direction: LayoutDirection) -> String {
    input
        .chars()
        .map(|ch| match direction {
            LayoutDirection::RussianToEnglish => russian_to_english(ch).unwrap_or(ch),
            LayoutDirection::EnglishToRussian => english_to_russian(ch).unwrap_or(ch),
        })
        .collect()
}

fn is_russian_letter(ch: char) -> bool {
    matches!(ch, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')
}

fn is_english_letter(ch: char) -> bool {
    ch.is_ascii_alphabetic()
}

fn russian_to_english(ch: char) -> Option<char> {
    Some(match ch {
        'й' => 'q',
        'ц' => 'w',
        'у' => 'e',
        'к' => 'r',
        'е' => 't',
        'н' => 'y',
        'г' => 'u',
        'ш' => 'i',
        'щ' => 'o',
        'з' => 'p',
        'х' => '[',
        'ъ' => ']',
        'ф' => 'a',
        'ы' => 's',
        'в' => 'd',
        'а' => 'f',
        'п' => 'g',
        'р' => 'h',
        'о' => 'j',
        'л' => 'k',
        'д' => 'l',
        'ж' => ';',
        'э' => '\'',
        'я' => 'z',
        'ч' => 'x',
        'с' => 'c',
        'м' => 'v',
        'и' => 'b',
        'т' => 'n',
        'ь' => 'm',
        'б' => ',',
        'ю' => '.',
        'ё' => '`',
        ',' => '?',
        '.' => '/',
        'Й' => 'Q',
        'Ц' => 'W',
        'У' => 'E',
        'К' => 'R',
        'Е' => 'T',
        'Н' => 'Y',
        'Г' => 'U',
        'Ш' => 'I',
        'Щ' => 'O',
        'З' => 'P',
        'Х' => '[',
        'Ъ' => ']',
        'Ф' => 'A',
        'Ы' => 'S',
        'В' => 'D',
        'А' => 'F',
        'П' => 'G',
        'Р' => 'H',
        'О' => 'J',
        'Л' => 'K',
        'Д' => 'L',
        'Ж' => ':',
        'Э' => '"',
        'Я' => 'Z',
        'Ч' => 'X',
        'С' => 'C',
        'М' => 'V',
        'И' => 'B',
        'Т' => 'N',
        'Ь' => 'M',
        'Б' => '<',
        'Ю' => '>',
        'Ё' => '~',
        _ => return None,
    })
}

fn english_to_russian(ch: char) -> Option<char> {
    Some(match ch {
        'q' => 'й',
        'w' => 'ц',
        'e' => 'у',
        'r' => 'к',
        't' => 'е',
        'y' => 'н',
        'u' => 'г',
        'i' => 'ш',
        'o' => 'щ',
        'p' => 'з',
        '[' => 'х',
        ']' => 'ъ',
        'a' => 'ф',
        's' => 'ы',
        'd' => 'в',
        'f' => 'а',
        'g' => 'п',
        'h' => 'р',
        'j' => 'о',
        'k' => 'л',
        'l' => 'д',
        ';' => 'ж',
        '\'' => 'э',
        'z' => 'я',
        'x' => 'ч',
        'c' => 'с',
        'v' => 'м',
        'b' => 'и',
        'n' => 'т',
        'm' => 'ь',
        ',' => 'б',
        '.' => 'ю',
        '`' => 'ё',
        '?' => ',',
        '/' => '.',
        'Q' => 'Й',
        'W' => 'Ц',
        'E' => 'У',
        'R' => 'К',
        'T' => 'Е',
        'Y' => 'Н',
        'U' => 'Г',
        'I' => 'Ш',
        'O' => 'Щ',
        'P' => 'З',
        'A' => 'Ф',
        'S' => 'Ы',
        'D' => 'В',
        'F' => 'А',
        'G' => 'П',
        'H' => 'Р',
        'J' => 'О',
        'K' => 'Л',
        'L' => 'Д',
        'Z' => 'Я',
        'X' => 'Ч',
        'C' => 'С',
        'V' => 'М',
        'B' => 'И',
        'N' => 'Т',
        'M' => 'Ь',
        '{' => 'Х',
        '}' => 'Ъ',
        ':' => 'Ж',
        '"' => 'Э',
        '<' => 'Б',
        '>' => 'Ю',
        '~' => 'Ё',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_english_mistyped_russian_to_russian() {
        assert_eq!(convert_layout_text("k.,jdm"), "любовь");
    }

    #[test]
    fn converts_russian_word_to_english() {
        assert_eq!(convert_layout_text("привет"), "ghbdtn");
    }

    #[test]
    fn converts_punctuation_mapping() {
        assert_eq!(convert_layout_text("четыре,"), "xtnsht?");
    }

    #[test]
    fn converts_selected_text_token_by_token() {
        assert_eq!(
            convert_selected_text("раз два три. xtnsht?"),
            "hfp ldf nhb/ четыре,"
        );
    }
}
