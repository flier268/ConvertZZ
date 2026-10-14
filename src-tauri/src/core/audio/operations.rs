//! 把標籤操作展開成既有計畫裡的欄位更新與容器移除。
//!
//! 來源一律讀這次掃描到的原始欄位，不讀前面規則寫進目標的結果。
//! 所以「先移除 APEv2，再從 APEv2 合併」仍然用得到移除前的文字，順序是確定的。
//! 同一個容器若先移除、後面又寫入，寫入蓋過移除；先寫入再移除，則最後移除整個容器。
//! `fillEmpty` 看的是原始目標再加上更早規則寫入後的文字，不看簡繁轉換的結果。
//! 來源是空字串時略過，避免把目標清成空白。沒指定衝突策略時，合併預設只補空白。

use super::super::types::{
    AudioContainer, AudioContainerRemoval, AudioFieldWarning, AudioFieldWarningCode, AudioTagField,
    AudioTagFile, AudioTagPlanRequest, TagAction, TagOnConflict, TagOperation, TextEncoding,
};
use super::{field_id, fit_encoded_text, label_for, read_tagged, PreparedAudio};
use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const STANDARD_FIELDS: &[&str] = &[
    "title", "artist", "album", "year", "track", "comment", "genre",
];

/// ID3v1 類型代碼。與 id3 crate 的 v1 清單相同，用來做名稱與代碼互轉。
const ID3V1_GENRES: &[&str] = &[
    "Blues",
    "Classic Rock",
    "Country",
    "Dance",
    "Disco",
    "Funk",
    "Grunge",
    "Hip-Hop",
    "Jazz",
    "Metal",
    "New Age",
    "Oldies",
    "Other",
    "Pop",
    "R&B",
    "Rap",
    "Reggae",
    "Rock",
    "Techno",
    "Industrial",
    "Alternative",
    "Ska",
    "Death Metal",
    "Pranks",
    "Soundtrack",
    "Euro-Techno",
    "Ambient",
    "Trip-Hop",
    "Vocal",
    "Jazz+Funk",
    "Fusion",
    "Trance",
    "Classical",
    "Instrumental",
    "Acid",
    "House",
    "Game",
    "Sound Clip",
    "Gospel",
    "Noise",
    "Alternative Rock",
    "Bass",
    "Soul",
    "Punk",
    "Space",
    "Meditative",
    "Instrumental Pop",
    "Instrumental Rock",
    "Ethnic",
    "Gothic",
    "Darkwave",
    "Techno-Industrial",
    "Electronic",
    "Pop-Folk",
    "Eurodance",
    "Dream",
    "Southern Rock",
    "Comedy",
    "Cult",
    "Gangsta",
    "Top 40",
    "Christian Rap",
    "Pop/Funk",
    "Jungle",
    "Native US",
    "Cabaret",
    "New Wave",
    "Psychadelic",
    "Rave",
    "Showtunes",
    "Trailer",
    "Lo-Fi",
    "Tribal",
    "Acid Punk",
    "Acid Jazz",
    "Polka",
    "Retro",
    "Musical",
    "Rock & Roll",
    "Hard Rock",
    "Folk",
    "Folk-Rock",
    "National Folk",
    "Swing",
    "Fast Fusion",
    "Bebob",
    "Latin",
    "Revival",
    "Celtic",
    "Bluegrass",
    "Avantgarde",
    "Gothic Rock",
    "Progressive Rock",
    "Psychedelic Rock",
    "Symphonic Rock",
    "Slow Rock",
    "Big Band",
    "Chorus",
    "Easy Listening",
    "Acoustic",
    "Humour",
    "Speech",
    "Chanson",
    "Opera",
    "Chamber Music",
    "Sonata",
    "Symphony",
    "Booty Bass",
    "Primus",
    "Porn Groove",
    "Satire",
    "Slow Jam",
    "Club",
    "Tango",
    "Samba",
    "Folklore",
    "Ballad",
    "Power Ballad",
    "Rhytmic Soul",
    "Freestyle",
    "Duet",
    "Punk Rock",
    "Drum Solo",
    "Acapella",
    "Euro-House",
    "Dance Hall",
    "Goa",
    "Drum & Bass",
    "Club-House",
    "Hardcore",
    "Terror",
    "Indie",
    "BritPop",
    "Negerpunk",
    "Polsk Punk",
    "Beat",
    "Christian Gangsta",
    "Heavy Metal",
    "Black Metal",
    "Crossover",
    "Contemporary C",
    "Christian Rock",
    "Merengue",
    "Salsa",
    "Thrash Metal",
    "Anime",
    "JPop",
    "SynthPop",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Touch {
    Write,
    Remove,
}

pub(super) fn removal_field_id(container: AudioContainer) -> String {
    format!("remove:{}", container_token(container))
}

fn container_token(container: AudioContainer) -> &'static str {
    match container {
        AudioContainer::Id3v1 => "id3v1",
        AudioContainer::Id3v2 => "id3v2",
        AudioContainer::Apev2 => "apev2",
        AudioContainer::VorbisComment => "vorbis-comment",
    }
}

pub(super) fn expand_tag_operations(
    request: &AudioTagPlanRequest,
    file: &mut AudioTagFile,
    prepared: &mut Option<PreparedAudio>,
    warnings: &mut Vec<String>,
) -> Result<(), super::super::error::CoreError> {
    if request.tag_operations.is_empty() || file.warning.is_some() {
        return Ok(());
    }
    let original = original_values(file);
    let present = containers_of(&original);
    let mut working = original.clone();
    let mut touches: HashMap<AudioContainer, Touch> = HashMap::new();
    let mut removals = Vec::new();
    file.removals.clear();

    let mut rule_index = 0usize;
    for operation in &request.tag_operations {
        let manual = !operation.paths.is_empty();
        if manual
            && !operation
                .paths
                .iter()
                .any(|path| super::paths_match(path, &file.path))
        {
            continue;
        }
        let origin = if manual {
            "manual".to_string()
        } else {
            rule_index += 1;
            format!("rule {rule_index}")
        };
        if !operation.enabled {
            continue;
        }
        match operation.action {
            TagAction::Remove => expand_remove(
                file,
                operation,
                &origin,
                &present,
                &mut touches,
                &mut removals,
                warnings,
            ),
            TagAction::Merge => expand_transfer(
                request,
                file,
                operation,
                &origin,
                &original,
                &mut working,
                &mut touches,
                warnings,
            )?,
        }
    }

    let mut remove_containers = HashSet::new();
    for removal in &removals {
        if removal.selected && touches.get(&removal.container) == Some(&Touch::Remove) {
            remove_containers.insert(removal.container);
        }
    }
    for container in &remove_containers {
        for field in &mut file.fields {
            if field.container == *container {
                field.converted_values = None;
                field.origin = None;
                field.truncated = false;
                field.truncation = None;
                field.warnings.clear();
                field.selected = false;
            }
        }
    }
    file.removals = removals;

    let rule_fields: Vec<(String, Vec<String>)> = file
        .fields
        .iter()
        .filter(|field| {
            field
                .origin
                .as_deref()
                .is_some_and(|origin| origin.starts_with("rule ") || origin == "manual")
                && field.selected
                && !remove_containers.contains(&field.container)
        })
        .filter_map(|field| {
            field
                .converted_values
                .as_ref()
                .map(|values| (field_id(field.container, &field.key), values.clone()))
        })
        .collect();
    if rule_fields.is_empty() && remove_containers.is_empty() {
        return Ok(());
    }
    let original_picture_count = if file.has_cover_art {
        super::picture_count(Path::new(&file.path))?
    } else {
        0
    };
    let prepared = prepared.get_or_insert_with(|| PreparedAudio {
        path: Path::new(&file.path).to_path_buf(),
        format: file.format,
        updates: HashMap::new(),
        selected_containers: HashSet::new(),
        rewrite_containers: HashSet::new(),
        remove_containers: HashSet::new(),
        request: request.clone(),
        original_picture_count,
    });
    for container in &remove_containers {
        prepared
            .updates
            .retain(|identifier, _| super::split_field_id(identifier).0 != *container);
        prepared.selected_containers.remove(container);
    }
    for (identifier, values) in rule_fields {
        let container = super::split_field_id(&identifier).0;
        prepared.updates.insert(identifier, values);
        prepared.selected_containers.insert(container);
    }
    prepared.remove_containers = remove_containers;
    Ok(())
}

fn expand_remove(
    file: &AudioTagFile,
    operation: &TagOperation,
    origin: &str,
    present: &HashSet<AudioContainer>,
    touches: &mut HashMap<AudioContainer, Touch>,
    removals: &mut Vec<AudioContainerRemoval>,
    warnings: &mut Vec<String>,
) {
    let Some(container) = operation.source.or(operation.target) else {
        warnings.push(format!("{}：移除規則沒有指定標籤。", file.path));
        return;
    };
    if !matches!(
        container,
        AudioContainer::Id3v1 | AudioContainer::Id3v2 | AudioContainer::Apev2
    ) {
        warnings.push(format!(
            "{}：第一版不能移除 {}。",
            file.path,
            container_token(container)
        ));
        return;
    }
    if !present.contains(&container) {
        return;
    }
    let hidden = count_hidden(Path::new(&file.path), container, &file.fields);
    let remaining = present
        .iter()
        .filter(|item| touches.get(item) != Some(&Touch::Remove))
        .count();
    let only_tag = remaining <= 1;
    let refused = only_tag.then(|| "這是檔案唯一的標籤，已拒絕移除。".to_string());
    if let Some(message) = &refused {
        warnings.push(format!("{}：{message}", file.path));
    } else {
        touches.insert(container, Touch::Remove);
    }
    if hidden > 0 {
        warnings.push(format!(
            "{}：移除 {} 含 {hidden} 個隱藏欄位。",
            file.path,
            container_token(container)
        ));
    }
    removals.retain(|item| item.container != container);
    removals.push(AudioContainerRemoval {
        container,
        hidden_field_count: hidden,
        origin: origin.to_string(),
        selected: refused.is_none(),
        refused,
    });
}

fn expand_transfer(
    request: &AudioTagPlanRequest,
    file: &mut AudioTagFile,
    operation: &TagOperation,
    origin: &str,
    original: &HashMap<(AudioContainer, String), String>,
    working: &mut HashMap<(AudioContainer, String), String>,
    touches: &mut HashMap<AudioContainer, Touch>,
    warnings: &mut Vec<String>,
) -> Result<(), super::super::error::CoreError> {
    let (Some(source), Some(target)) = (operation.source, operation.target) else {
        warnings.push(format!("{}：合併規則需要來源和目標。", file.path));
        return Ok(());
    };
    if !transfer_supported(source, target) {
        warnings.push(format!(
            "{}：第一版不能從 {} 合併到 {}。",
            file.path,
            container_token(source),
            container_token(target)
        ));
        return Ok(());
    }
    let conflict = operation.on_conflict.unwrap_or(TagOnConflict::FillEmpty);
    let keys = if operation.fields.is_empty() {
        STANDARD_FIELDS
            .iter()
            .map(|key| (*key).to_string())
            .collect()
    } else {
        operation.fields.clone()
    };
    for key in keys {
        let Some(source_value) = original.get(&(source, key.clone())) else {
            if !STANDARD_FIELDS.contains(&key.as_str()) {
                note_unmappable(file, target, &key, origin, warnings);
            }
            continue;
        };
        if source_value.trim().is_empty() {
            continue;
        }
        if !field_supported(target, &key) {
            note_unmappable(file, target, &key, origin, warnings);
            continue;
        }
        let current = working
            .get(&(target, key.clone()))
            .cloned()
            .unwrap_or_default();
        if conflict == TagOnConflict::FillEmpty && !current.trim().is_empty() {
            continue;
        }
        let mut text = source_value.clone();
        if key == "genre" {
            match map_genre(source, target, &text) {
                GenreMap::Value(value) => text = value,
                GenreMap::Unmappable(message) => {
                    note_genre(file, target, origin, &message, warnings);
                    continue;
                }
            }
        }
        let mut field_warnings = Vec::new();
        if target == AudioContainer::Id3v1
            && matches!(
                key.as_str(),
                "title" | "artist" | "album" | "year" | "comment"
            )
        {
            let limit = id3v1_limit(&key, working);
            let fit = fit_encoded_text(&text, limit, request.id3v1_output_encoding)?;
            if fit.unrepresentable > 0 {
                field_warnings.push(AudioFieldWarning {
                    code: AudioFieldWarningCode::Unrepresentable,
                    message: format!(
                        "{} 字 {} 無法表示",
                        fit.unrepresentable,
                        encoding_label(request.id3v1_output_encoding)
                    ),
                    from_bytes: None,
                    to_bytes: None,
                    count: Some(fit.unrepresentable),
                });
            }
            if fit.truncated {
                field_warnings.push(AudioFieldWarning {
                    code: AudioFieldWarningCode::Truncated,
                    message: format!("截斷 {}→{} B", fit.from_bytes, fit.to_bytes),
                    from_bytes: Some(fit.from_bytes),
                    to_bytes: Some(fit.to_bytes),
                    count: None,
                });
            }
            text = fit.text;
        }
        working.insert((target, key.clone()), text.clone());
        touches.insert(target, Touch::Write);
        let changed = current != text;
        if !changed && field_warnings.is_empty() {
            continue;
        }
        let field = ensure_field(file, target, &key);
        field.values = if field.values.is_empty() {
            vec![current]
        } else {
            field.values.clone()
        };
        field.converted_values = Some(vec![text]);
        field.selected = true;
        field.origin = Some(origin.to_string());
        field.truncated = field_warnings
            .iter()
            .any(|warning| warning.code == AudioFieldWarningCode::Truncated);
        field.truncation = field_warnings.iter().find_map(|warning| {
            (warning.code == AudioFieldWarningCode::Truncated).then(|| warning.message.clone())
        });
        field.warnings = field_warnings;
    }
    Ok(())
}

fn note_unmappable(
    file: &mut AudioTagFile,
    target: AudioContainer,
    key: &str,
    origin: &str,
    warnings: &mut Vec<String>,
) {
    let message = format!(
        "「{}」無法對應到 {}",
        label_for(key),
        container_token(target)
    );
    warnings.push(format!("{}：{message}", file.path));
    let field = ensure_field(file, target, key);
    field.selected = false;
    field.origin = Some(origin.to_string());
    field.warnings = vec![AudioFieldWarning {
        code: AudioFieldWarningCode::Unmappable,
        message,
        from_bytes: None,
        to_bytes: None,
        count: None,
    }];
}

fn note_genre(
    file: &mut AudioTagFile,
    target: AudioContainer,
    origin: &str,
    message: &str,
    warnings: &mut Vec<String>,
) {
    warnings.push(format!("{}：{message}", file.path));
    let field = ensure_field(file, target, "genre");
    field.selected = false;
    field.origin = Some(origin.to_string());
    field.warnings = vec![AudioFieldWarning {
        code: AudioFieldWarningCode::Unmappable,
        message: message.to_string(),
        from_bytes: None,
        to_bytes: None,
        count: None,
    }];
}

fn ensure_field<'a>(
    file: &'a mut AudioTagFile,
    container: AudioContainer,
    key: &str,
) -> &'a mut AudioTagField {
    if let Some(index) = file
        .fields
        .iter()
        .position(|field| field.container == container && field.key == key)
    {
        return &mut file.fields[index];
    }
    file.fields.push(AudioTagField {
        key: key.to_string(),
        label: label_for(key).to_string(),
        container,
        values: Vec::new(),
        converted_values: None,
        selected: false,
        truncated: false,
        truncation: None,
        origin: None,
        warnings: Vec::new(),
    });
    file.fields.last_mut().expect("just pushed")
}

fn original_values(file: &AudioTagFile) -> HashMap<(AudioContainer, String), String> {
    let mut map = HashMap::new();
    for field in &file.fields {
        if let Some(value) = field.values.first() {
            map.insert((field.container, field.key.clone()), value.clone());
        }
    }
    map
}

fn containers_of(values: &HashMap<(AudioContainer, String), String>) -> HashSet<AudioContainer> {
    values.keys().map(|(container, _)| *container).collect()
}

fn transfer_supported(source: AudioContainer, target: AudioContainer) -> bool {
    matches!(
        (source, target),
        (AudioContainer::Id3v2, AudioContainer::Id3v1)
            | (AudioContainer::Id3v1, AudioContainer::Id3v2)
            | (AudioContainer::Apev2, AudioContainer::Id3v2)
    )
}

fn field_supported(target: AudioContainer, key: &str) -> bool {
    if target == AudioContainer::Id3v1 {
        return STANDARD_FIELDS.contains(&key);
    }
    STANDARD_FIELDS.contains(&key) || matches!(key, "albumArtist" | "composer" | "lyrics")
}

fn id3v1_limit(key: &str, working: &HashMap<(AudioContainer, String), String>) -> usize {
    match key {
        "year" => 4,
        "comment" => {
            let track = working
                .get(&(AudioContainer::Id3v1, "track".into()))
                .and_then(|value| value.parse::<u8>().ok())
                .unwrap_or(0);
            if track > 0 {
                28
            } else {
                30
            }
        }
        _ => 30,
    }
}

enum GenreMap {
    Value(String),
    Unmappable(String),
}

fn map_genre(source: AudioContainer, target: AudioContainer, value: &str) -> GenreMap {
    if target == AudioContainer::Id3v1 {
        if let Some(code) = genre_code(value) {
            return GenreMap::Value(code.to_string());
        }
        return GenreMap::Unmappable(format!("類型「{value}」沒有對應的 ID3v1 代碼"));
    }
    if source == AudioContainer::Id3v1 {
        if let Ok(code) = value.trim().parse::<u8>() {
            if let Some(name) = ID3V1_GENRES.get(code as usize) {
                return GenreMap::Value((*name).to_string());
            }
            return GenreMap::Unmappable(format!("ID3v1 類型代碼 {code} 沒有名稱"));
        }
    }
    GenreMap::Value(value.to_string())
}

fn genre_code(value: &str) -> Option<u8> {
    let trimmed = value.trim();
    if let Ok(code) = trimmed.parse::<u8>() {
        if ID3V1_GENRES.get(code as usize).is_some() || trimmed == "255" {
            return Some(code);
        }
    }
    let lower = trimmed.to_ascii_lowercase();
    ID3V1_GENRES
        .iter()
        .position(|name| name.eq_ignore_ascii_case(&lower) || name.to_ascii_lowercase() == lower)
        .map(|index| index as u8)
}

fn encoding_label(encoding: TextEncoding) -> &'static str {
    match encoding {
        TextEncoding::Big5 => "Big5",
        TextEncoding::Gbk => "GBK",
        TextEncoding::ShiftJis => "Shift-JIS",
        TextEncoding::Utf8 | TextEncoding::Utf8Bom => "UTF-8",
        _ => "目標編碼",
    }
}

pub(super) fn lofty_item_shown(key: ItemKey) -> bool {
    matches!(
        key,
        ItemKey::TrackTitle
            | ItemKey::TrackArtist
            | ItemKey::AlbumTitle
            | ItemKey::Genre
            | ItemKey::Comment
            | ItemKey::AlbumArtist
            | ItemKey::Composer
            | ItemKey::Lyrics
            | ItemKey::UnsyncLyrics
            | ItemKey::TrackNumber
            | ItemKey::RecordingDate
            | ItemKey::Year
    )
}

pub(super) fn container_of_lofty(tag_type: lofty::tag::TagType) -> Option<AudioContainer> {
    match tag_type {
        lofty::tag::TagType::Id3v1 => Some(AudioContainer::Id3v1),
        lofty::tag::TagType::Id3v2 => Some(AudioContainer::Id3v2),
        lofty::tag::TagType::Ape => Some(AudioContainer::Apev2),
        lofty::tag::TagType::VorbisComments => Some(AudioContainer::VorbisComment),
        _ => None,
    }
}

/// 給測試與掃描確認用。讀取失敗就當沒有隱藏欄位，不讓預覽整檔失敗。
/// ID3v2 只計未顯示的文字框：多出來的 COMM、歌詞，以及掃描略過的無文字 T*。
/// 封面等非文字框不計。
pub(super) fn count_hidden(
    path: &Path,
    container: AudioContainer,
    fields: &[AudioTagField],
) -> usize {
    match container {
        AudioContainer::Id3v1 => 0,
        AudioContainer::Id3v2 => count_hidden_id3v2(path, fields),
        AudioContainer::Apev2 => count_hidden_ape(path),
        AudioContainer::VorbisComment => 0,
    }
}

fn count_hidden_id3v2(path: &Path, fields: &[AudioTagField]) -> usize {
    let Ok(tag) = id3::Tag::read_from_path(path) else {
        return 0;
    };
    let mut keys: Vec<&str> = fields
        .iter()
        .filter(|field| field.container == AudioContainer::Id3v2)
        .map(|field| field.key.as_str())
        .collect();
    tag.frames()
        .filter(|frame| id3_countable_text_frame(frame))
        .filter(|frame| !take_shown_id3_field(frame, &mut keys))
        .count()
}

/// 隱藏欄位只看文字框：T*（含沒有純文字內容的 TIPL）、COMM、歌詞與網址文字。
/// APIC 這類二進位框不在這裡。
fn id3_countable_text_frame(frame: &id3::frame::Frame) -> bool {
    let id = frame.id();
    id.starts_with('T') || matches!(id, "COMM" | "COM" | "USLT" | "SYLT" | "USER" | "WXXX")
}

/// 這一框若對得上掃描產出的某個欄位，就把那個欄位用掉。
/// 同名欄位只對一框，所以第二則 COMM 不會再被 `comment` 蓋掉。
fn take_shown_id3_field(frame: &id3::frame::Frame, keys: &mut Vec<&str>) -> bool {
    let Some(wanted) = shown_id3_field_key(frame) else {
        return false;
    };
    let Some(index) = keys.iter().position(|key| *key == wanted) else {
        return false;
    };
    keys.remove(index);
    true
}

/// 與 `scan_mp3` 相同的對應：標準欄位各收一框；其餘只收有文字的 T*。
fn shown_id3_field_key(frame: &id3::frame::Frame) -> Option<String> {
    let standard = match frame.id() {
        "TIT2" | "TT2" => Some("title"),
        "TPE1" | "TP1" => Some("artist"),
        "TALB" | "TAL" => Some("album"),
        "TYER" | "TDRC" | "TYE" => Some("year"),
        "TRCK" | "TRK" => Some("track"),
        "TCON" | "TCO" => Some("genre"),
        "COMM" => Some("comment"),
        _ => None,
    };
    if let Some(key) = standard {
        return Some(key.to_string());
    }
    let id = frame.id();
    if id.starts_with('T') && (3..=4).contains(&id.len()) {
        if let Some(extended) = frame.content().extended_text() {
            return Some(format!("frame:{id}:{}", extended.description));
        }
        if frame.content().text().is_some() {
            return Some(format!("frame:{id}"));
        }
    }
    None
}

fn count_hidden_ape(path: &Path) -> usize {
    let Ok(tagged) = read_tagged(path) else {
        return 0;
    };
    tagged
        .tags()
        .iter()
        .filter(|tag| tag.tag_type() == lofty::tag::TagType::Ape)
        .flat_map(|tag| tag.items())
        .filter(|item| !lofty_item_shown(item.key()))
        .count()
}
