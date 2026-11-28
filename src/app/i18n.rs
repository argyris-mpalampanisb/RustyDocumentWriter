//! Interface language. The English text is the key: `tr("Paste")` returns the
//! Greek string when Greek is active and the English one otherwise (also for
//! anything not translated yet), so English needs no table at all.
//!
//! A few English words mean different things in different places ("Home" is
//! both a ribbon tab and a File page); those keys carry a context before a
//! `\u{4}` separator, e.g. `"file\u{4}Home"`, which is dropped for display.
//!
//! Terms follow the Greek version of Microsoft Word where it has one.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    El,
}

/// Languages offered in Options: (language, settings code, name in that language).
pub const LANGS: &[(Lang, &str, &str)] = &[(Lang::En, "en", "English"), (Lang::El, "el", "Ελληνικά")];

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == Lang::El as u8 { Lang::El } else { Lang::En }
}

pub fn from_code(code: &str) -> Option<Lang> {
    LANGS.iter().find(|(_, c, _)| *c == code).map(|(l, _, _)| *l)
}

/// The language the system asks for, if we have it (else English).
pub fn system() -> Lang {
    static SYSTEM: OnceLock<Lang> = OnceLock::new();
    *SYSTEM.get_or_init(detect_system)
}

fn detect_system() -> Lang {
    // LC_ALL / LC_MESSAGES win over the desktop locale on Unix, as in gettext.
    let env = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().filter_map(|v| std::env::var(v).ok()).find(|v| !v.is_empty() && v != "C" && v != "POSIX");
    let loc = env.or_else(sys_locale::get_locale).unwrap_or_default().to_ascii_lowercase();
    if loc.starts_with("el") { Lang::El } else { Lang::En }
}

/// Resolve the `language` setting ("system", "en", "el").
pub fn resolve(setting: &str) -> Lang {
    from_code(setting).unwrap_or_else(system)
}

fn strip_context(s: &str) -> &str {
    s.rsplit_once('\u{4}').map_or(s, |(_, text)| text)
}

fn greek() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| EL.iter().copied().collect())
}

/// Translate an interface string.
pub fn tr(s: &str) -> &str {
    match current() {
        Lang::El => greek().get(s).copied().unwrap_or_else(|| strip_context(s)),
        Lang::En => strip_context(s),
    }
}

/// Translate a template and fill it: `{}` takes the next argument, `{0}`, `{1}`… a
/// given one (so a translation can reorder them).
pub fn tf(template: &str, args: &[&dyn Display]) -> String {
    fill(tr(template), args)
}

fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) if after[..close].chars().all(|c| c.is_ascii_digit()) => {
                let idx = if close == 0 {
                    next += 1;
                    next - 1
                } else {
                    after[..close].parse().unwrap_or(usize::MAX)
                };
                if let Some(a) = args.get(idx) {
                    out.push_str(&a.to_string());
                }
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// English → Greek.
pub const EL: &[(&str, &str)] = &[
    // Ribbon tabs and title bar
    ("File", "Αρχείο"),
    ("Home", "Κεντρική"),
    ("Insert", "Εισαγωγή"),
    ("Design", "Σχεδίαση"),
    ("Layout", "Διάταξη"),
    ("References", "Αναφορές"),
    ("Mailings", "Στοιχεία αλληλογραφίας"),
    ("Review", "Αναθεώρηση"),
    ("View", "Προβολή"),
    ("Help", "Βοήθεια"),
    ("Share", "Κοινή χρήση"),
    ("Save a copy to share", "Αποθήκευση αντιγράφου για κοινή χρήση"),
    ("Keep the ribbon open (Ctrl+F1)", "Μόνιμη εμφάνιση της κορδέλας (Ctrl+F1)"),
    ("Collapse the ribbon (Ctrl+F1)", "Σύμπτυξη της κορδέλας (Ctrl+F1)"),
    ("AutoSave", "Αυτόματη αποθήκευση"),
    ("AutoSave needs cloud storage — use Save (Ctrl+S)", "Η αυτόματη αποθήκευση χρειάζεται αποθηκευτικό χώρο στο cloud — χρησιμοποιήστε την Αποθήκευση (Ctrl+S)"),
    ("Save (Ctrl+S)", "Αποθήκευση (Ctrl+S)"),
    ("Undo (Ctrl+Z)", "Αναίρεση (Ctrl+Z)"),
    ("Redo (Ctrl+Y)", "Ακύρωση αναίρεσης (Ctrl+Y)"),
    ("Search", "Αναζήτηση"),
    // Colour pickers
    ("Theme Colors", "Χρώματα θέματος"),
    ("Standard Colors", "Βασικά χρώματα"),
    ("Automatic", "Αυτόματο"),
    ("Automatic Color", "Αυτόματο χρώμα"),
    ("No Color", "Χωρίς χρώμα"),
    ("Underline Color", "Χρώμα υπογράμμισης"),
    ("No Underline", "Χωρίς υπογράμμιση"),
    ("Silver", "Ασημί"),
    // Underline styles
    ("Single", "Μονή"),
    ("Words only", "Μόνο λέξεις"),
    ("Double", "Διπλή"),
    ("Thick", "Έντονη"),
    ("Dotted", "Διάστικτη"),
    ("Dashed", "Διακεκομμένη"),
    ("Dot-dash", "Τελεία-παύλα"),
    ("Dot-dot-dash", "Τελεία-τελεία-παύλα"),
    ("Wave", "Κυματιστή"),
    ("Heavy wave", "Έντονη κυματιστή"),
    ("Double wave", "Διπλή κυματιστή"),
    // Text effects menu
    ("Text Themes", "Θέματα κειμένου"),
    ("Effects", "Εφέ"),
    ("Outline", "Περίγραμμα"),
    ("Shadow", "Σκιά"),
    ("Emboss", "Ανάγλυφο"),
    ("Engrave", "Χαρακτικό"),
    ("Small Caps    Ctrl+Shift+K", "Μικρά κεφαλαία    Ctrl+Shift+K"),
    ("All Caps    Ctrl+Shift+A", "Όλα κεφαλαία    Ctrl+Shift+A"),
    ("Double Strikethrough", "Διπλή διακριτή διαγραφή"),
    ("Remove Text Effects", "Κατάργηση εφέ κειμένου"),
    // Text theme names
    ("Headline", "Πρωτοσέλιδο"),
    ("Hollow", "Κούφιο"),
    ("Neon", "Νέον"),
    ("Embossed", "Ανάγλυφο"),
    ("Engraved", "Χαραγμένο"),
    ("Marquee", "Μαρκίζα"),
    ("Signature", "Υπογραφή"),
    ("Whisper", "Ψίθυρος"),
    ("Struck Out", "Διαγραμμένο"),
    ("Highlighter", "Μαρκαδόρος"),
    ("Typewriter", "Γραφομηχανή"),
    ("Stamp", "Σφραγίδα"),
    // 3D shapes
    ("3D Shapes", "Τρισδιάστατα σχήματα"),
    ("Shapes are inserted as pictures, so they look the same in any word processor.", "Τα σχήματα εισάγονται ως εικόνες, ώστε να εμφανίζονται ίδια σε κάθε επεξεργαστή κειμένου."),
    ("Cube", "Κύβος"),
    ("Sphere", "Σφαίρα"),
    ("Cylinder", "Κύλινδρος"),
    ("Cone", "Κώνος"),
    ("Pyramid", "Πυραμίδα"),
    ("Torus", "Τόρος"),
    ("Capsule", "Κάψουλα"),
    ("Hexagon", "Εξάγωνο"),
    ("Gem", "Πολύτιμος λίθος"),
    ("Ring", "Δαχτυλίδι"),
    ("Star", "Αστέρι"),
    ("Heart", "Καρδιά"),
    ("Arrow", "Βέλος"),
    ("Lightning", "Κεραυνός"),
    ("Plus", "Συν"),
    ("Check Mark", "Σημάδι ελέγχου"),
    ("Speech Bubble", "Συννεφάκι ομιλίας"),
    ("Crown", "Στέμμα"),
    // Home: Clipboard, Font
    ("Clipboard", "Πρόχειρο"),
    ("Paste", "Επικόλληση"),
    ("Cut (Ctrl+X)", "Αποκοπή (Ctrl+X)"),
    ("Copy (Ctrl+C)", "Αντιγραφή (Ctrl+C)"),
    ("Format Painter", "Πινέλο μορφοποίησης"),
    ("Font", "Γραμματοσειρά"),
    ("{} (Headings)", "{} (Επικεφαλίδες)"),
    ("{} (Body)", "{} (Σώμα)"),
    ("Increase Font Size (Ctrl+])", "Αύξηση μεγέθους γραμματοσειράς (Ctrl+])"),
    ("Decrease Font Size (Ctrl+[)", "Μείωση μεγέθους γραμματοσειράς (Ctrl+[)"),
    ("Change Case", "Αλλαγή πεζών-κεφαλαίων"),
    ("UPPERCASE", "ΚΕΦΑΛΑΙΑ"),
    ("lowercase", "πεζά"),
    ("Capitalize Each Word", "Κεφαλαίο το πρώτο γράμμα κάθε λέξης"),
    ("Clear All Formatting", "Απαλοιφή όλης της μορφοποίησης"),
    ("Bold (Ctrl+B)", "Έντονα (Ctrl+B)"),
    ("Italic (Ctrl+I)", "Πλάγια (Ctrl+I)"),
    ("Underline (Ctrl+U)", "Υπογράμμιση (Ctrl+U)"),
    ("Strikethrough", "Διακριτή διαγραφή"),
    ("Subscript (Ctrl+=)", "Δείκτης (Ctrl+=)"),
    ("Superscript (Ctrl+Shift++)", "Εκθέτης (Ctrl+Shift++)"),
    ("Text Effects and Themes", "Εφέ και θέματα κειμένου"),
    ("Text Highlight Color", "Χρώμα επισήμανσης κειμένου"),
    ("Font Color", "Χρώμα γραμματοσειράς"),
    // Home: Paragraph
    ("Paragraph", "Παράγραφος"),
    ("Bullets", "Κουκκίδες"),
    ("Numbering", "Αρίθμηση"),
    ("Multilevel List", "Λίστα πολλών επιπέδων"),
    ("Decrease Indent", "Μείωση εσοχής"),
    ("Increase Indent", "Αύξηση εσοχής"),
    ("Sort", "Ταξινόμηση"),
    ("Show/Hide ¶ (Ctrl+*)", "Εμφάνιση/Απόκρυψη ¶ (Ctrl+*)"),
    ("Align Left (Ctrl+L)", "Στοίχιση αριστερά (Ctrl+L)"),
    ("Center (Ctrl+E)", "Στοίχιση στο κέντρο (Ctrl+E)"),
    ("Align Right (Ctrl+R)", "Στοίχιση δεξιά (Ctrl+R)"),
    ("Justify (Ctrl+J)", "Πλήρης στοίχιση (Ctrl+J)"),
    ("Line and Paragraph Spacing", "Διάστιχο και απόσταση παραγράφων"),
    ("Remove Space After Paragraph", "Κατάργηση διαστήματος μετά από παράγραφο"),
    ("Add Space Before Paragraph", "Προσθήκη διαστήματος πριν από παράγραφο"),
    ("Shading", "Σκίαση"),
    ("Borders", "Περιγράμματα"),
    ("Outside Borders", "Εξωτερικά περιγράμματα"),
    ("No Border", "Χωρίς περίγραμμα"),
    // Home: Styles, Editing, Voice
    ("Styles", "Στυλ"),
    ("Normal", "Κανονικό"),
    ("No Spacing", "Χωρίς διάστιχο"),
    ("Heading 1", "Επικεφαλίδα 1"),
    ("Heading 2", "Επικεφαλίδα 2"),
    ("Title", "Τίτλος"),
    ("Subtitle", "Υπότιτλος"),
    ("Quote", "Απόσπασμα"),
    ("Editing", "Επεξεργασία"),
    ("Find", "Εύρεση"),
    ("Replace", "Αντικατάσταση"),
    ("Select", "Επιλογή"),
    ("Voice", "Φωνή"),
    ("Dictate", "Υπαγόρευση"),
    // Insert
    ("Pages", "Σελίδες"),
    ("Cover\nPage", "Σελίδα\nεξωφύλλου"),
    ("Blank\nPage", "Κενή\nσελίδα"),
    ("Page\nBreak", "Αλλαγή\nσελίδας"),
    ("Tables", "Πίνακες"),
    ("Table", "Πίνακας"),
    ("{}x{} Table", "Πίνακας {}x{}"),
    ("Insert Table", "Εισαγωγή πίνακα"),
    ("Insert Table… (3 x 3)", "Εισαγωγή πίνακα… (3 x 3)"),
    ("Illustrations", "Απεικονίσεις"),
    ("Pictures", "Εικόνες"),
    ("Shapes", "Σχήματα"),
    ("Icons", "Εικονίδια"),
    ("Chart", "Γράφημα"),
    ("Screenshot", "Στιγμιότυπο\nοθόνης"),
    ("Links", "Σύνδεσμοι"),
    ("Link", "Σύνδεση"),
    ("Bookmark", "Σελιδοδείκτης"),
    ("Cross-reference", "Παραπομπή"),
    ("This feature", "Αυτή η δυνατότητα"),
    ("Comments", "Σχόλια"),
    ("Comment", "Σχόλιο"),
    ("Header & Footer", "Κεφαλίδα & υποσέλιδο"),
    ("Header", "Κεφαλίδα"),
    ("Footer", "Υποσέλιδο"),
    ("Header & Footer gallery (double-click a header to edit it)", "Συλλογή κεφαλίδων και υποσέλιδων (κάντε διπλό κλικ σε μια κεφαλίδα για να την επεξεργαστείτε)"),
    ("Page\nNumber", "Αριθμός\nσελίδας"),
    ("Text", "Κείμενο"),
    ("Text\nBox", "Πλαίσιο\nκειμένου"),
    ("Quick Parts", "Γρήγορα τμήματα"),
    ("Drop Cap", "Αρχιγράμματο"),
    ("Signature Line", "Γραμμή υπογραφής"),
    ("Date & Time", "Ημερομηνία & ώρα"),
    ("Object", "Αντικείμενο"),
    ("Symbols", "Σύμβολα"),
    ("Equation", "Εξίσωση"),
    ("Symbol", "Σύμβολο"),
    // Insert ▸ Page Number
    ("Bottom of Page", "Κάτω μέρος σελίδας"),
    ("Top of Page", "Επάνω μέρος σελίδας"),
    ("Brackets", "Αγκύλες"),
    ("Thin Line", "Λεπτή γραμμή"),
    ("Thick Line", "Παχιά γραμμή"),
    ("Number format", "Μορφή αριθμού"),
    ("Remove Page Numbers", "Κατάργηση αριθμών σελίδας"),
    // WordArt
    ("Edit WordArt", "Επεξεργασία WordArt"),
    ("Insert WordArt", "Εισαγωγή WordArt"),
    ("Size", "Μέγεθος"),
    ("Style", "Στυλ"),
    ("Preview", "Προεπισκόπηση"),
    ("Your text here", "Το κείμενό σας εδώ"),
    ("Ocean Blue", "Μπλε του ωκεανού"),
    ("Gold 3D", "Χρυσό 3D"),
    ("Chrome", "Χρώμιο"),
    ("Fire", "Φωτιά"),
    ("Reflection", "Αντανάκλαση"),
    ("Retro 3D", "Ρετρό 3D"),
    ("Candy", "Καραμέλα"),
    ("Emerald 3D", "Σμαραγδί 3D"),
    ("Ice", "Πάγος"),
    ("Rainbow", "Ουράνιο τόξο"),
    // Design
    ("Document Formatting", "Μορφοποίηση εγγράφου"),
    ("Themes", "Θέματα"),
    ("Colors", "Χρώματα"),
    ("Fonts", "Γραμματοσειρές"),
    ("Paragraph Spacing", "Απόσταση παραγράφων"),
    ("Set as Default", "Ορισμός ως προεπιλογής"),
    ("Page Background", "Φόντο σελίδας"),
    ("Watermark", "Υδατογράφημα"),
    ("Page\nColor", "Χρώμα\nσελίδας"),
    ("Page\nBorders", "Περιγράμματα\nσελίδας"),
    // Layout
    ("Page Setup", "Διαμόρφωση σελίδας"),
    ("Margins", "Περιθώρια"),
    ("Normal  (1\" all)", "Κανονικά  (1\" παντού)"),
    ("Narrow  (0.5\" all)", "Στενά  (0,5\" παντού)"),
    ("Moderate  (1\" / 0.75\")", "Μέτρια  (1\" / 0,75\")"),
    ("Wide  (1\" / 2\")", "Φαρδιά  (1\" / 2\")"),
    ("Orientation", "Προσανατολισμός"),
    ("Portrait", "Κατακόρυφος"),
    ("Landscape", "Οριζόντιος"),
    ("Columns", "Στήλες"),
    ("One", "Μία"),
    ("Two", "Δύο"),
    ("Three", "Τρεις"),
    ("Breaks", "Αλλαγές"),
    ("Page", "Σελίδα"),
    ("Column", "Στήλη"),
    ("Section Break: Next Page", "Αλλαγή ενότητας: Επόμενη σελίδα"),
    ("Section Break: Continuous", "Αλλαγή ενότητας: Συνεχής"),
    ("Line Numbers", "Αριθμοί γραμμών"),
    ("Hyphenation", "Συλλαβισμός"),
    ("Indent", "Εσοχή"),
    ("Left:", "Αριστερά:"),
    ("Right:", "Δεξιά:"),
    ("Spacing", "Απόσταση"),
    ("Before:", "Πριν:"),
    ("After:", "Μετά:"),
    ("Arrange", "Τακτοποίηση"),
    ("Position", "Θέση"),
    ("Wrap\nText", "Αναδίπλωση\nκειμένου"),
    ("Bring\nForward", "Μεταφορά προς\nτα εμπρός"),
    ("Send\nBackward", "Μεταφορά προς\nτα πίσω"),
    ("Selection\nPane", "Παράθυρο\nεπιλογής"),
    ("Align", "Στοίχιση"),
    ("Group", "Ομαδοποίηση"),
    ("Rotate", "Περιστροφή"),
    // References
    ("Table of Contents", "Πίνακας περιεχομένων"),
    ("Table of\nContents", "Πίνακας\nπεριεχομένων"),
    ("Table of Contents generation", "Δημιουργία πίνακα περιεχομένων"),
    ("Add Text", "Προσθήκη κειμένου"),
    ("Update Table", "Ενημέρωση πίνακα"),
    ("Footnotes", "Υποσημειώσεις"),
    ("Insert\nFootnote", "Εισαγωγή\nυποσημείωσης"),
    ("Insert Footnote", "Εισαγωγή υποσημείωσης"),
    ("Insert Endnote", "Εισαγωγή σημείωσης τέλους"),
    ("Next Footnote", "Επόμενη υποσημείωση"),
    ("Show Notes", "Εμφάνιση σημειώσεων"),
    ("Citations & Bibliography", "Παραθέσεις & βιβλιογραφία"),
    ("Insert\nCitation", "Εισαγωγή\nπαράθεσης"),
    ("Manage Sources", "Διαχείριση πηγών"),
    ("Style: APA", "Στυλ: APA"),
    ("Bibliography", "Βιβλιογραφία"),
    ("Captions", "Λεζάντες"),
    ("Insert\nCaption", "Εισαγωγή\nλεζάντας"),
    ("Insert Table of Figures", "Εισαγωγή πίνακα εικόνων"),
    ("Index", "Ευρετήριο"),
    ("Mark\nEntry", "Σήμανση\nλήμματος"),
    ("Insert Index", "Εισαγωγή ευρετηρίου"),
    ("Update Index", "Ενημέρωση ευρετηρίου"),
    ("Table of Authorities", "Πίνακας νομικών παραπομπών"),
    ("Mark\nCitation", "Σήμανση\nπαράθεσης"),
    // Mailings
    ("Create", "Δημιουργία"),
    ("Envelopes", "Φάκελοι"),
    ("Labels", "Ετικέτες"),
    ("Start Mail Merge", "Έναρξη συγχώνευσης αλληλογραφίας"),
    ("Start Mail\nMerge", "Έναρξη συγχώνευσης\nαλληλογραφίας"),
    ("Select\nRecipients", "Επιλογή\nπαραληπτών"),
    ("Edit\nRecipient List", "Επεξεργασία\nλίστας παραληπτών"),
    ("Write & Insert Fields", "Σύνταξη & εισαγωγή πεδίων"),
    ("Highlight\nMerge Fields", "Επισήμανση πεδίων\nσυγχώνευσης"),
    ("Address\nBlock", "Μπλοκ\nδιεύθυνσης"),
    ("Greeting\nLine", "Γραμμή\nχαιρετισμού"),
    ("Insert Merge\nField", "Εισαγωγή πεδίου\nσυγχώνευσης"),
    ("Rules", "Κανόνες"),
    ("Match Fields", "Αντιστοίχιση πεδίων"),
    ("Update Labels", "Ενημέρωση ετικετών"),
    ("Preview Results", "Προεπισκόπηση αποτελεσμάτων"),
    ("Preview\nResults", "Προεπισκόπηση\nαποτελεσμάτων"),
    ("Finish", "Ολοκλήρωση"),
    ("Finish &\nMerge", "Ολοκλήρωση &\nσυγχώνευση"),
    // Review
    ("Proofing", "Γλωσσικός έλεγχος"),
    ("Spelling &\nGrammar", "Ορθογραφία &\nγραμματική"),
    ("Thesaurus", "Θησαυρός"),
    ("Word Count", "Καταμέτρηση λέξεων"),
    ("Speech", "Ομιλία"),
    ("Read\nAloud", "Ανάγνωση\nφωναχτά"),
    ("Accessibility", "Προσβασιμότητα"),
    ("Check\nAccessibility", "Έλεγχος\nπροσβασιμότητας"),
    ("Language", "Γλώσσα"),
    ("New Comment", "Νέο σχόλιο"),
    ("Delete", "Διαγραφή"),
    ("Previous", "Προηγούμενο"),
    ("Next", "Επόμενο"),
    ("Show Comments", "Εμφάνιση σχολίων"),
    ("Tracking", "Παρακολούθηση"),
    ("Track\nChanges", "Παρακολούθηση\nαλλαγών"),
    ("Simple Markup", "Απλή σήμανση"),
    ("Show Markup", "Εμφάνιση σήμανσης"),
    ("Reviewing Pane", "Παράθυρο αναθεώρησης"),
    ("Changes", "Αλλαγές"),
    ("Accept", "Αποδοχή"),
    ("Reject", "Απόρριψη"),
    ("Compare", "Σύγκριση"),
    ("Protect", "Προστασία"),
    // View
    ("Views", "Προβολές"),
    ("Read\nMode", "Λειτουργία\nανάγνωσης"),
    ("Print\nLayout", "Διάταξη\nεκτύπωσης"),
    ("Web\nLayout", "Διάταξη\nweb"),
    ("view\u{4}Outline", "Διάρθρωση"),
    ("Draft", "Πρόχειρο"),
    ("Dark Mode", "Σκούρα λειτουργία"),
    ("Switch\nModes", "Εναλλαγή\nλειτουργιών"),
    ("Show the page in dark colours", "Εμφάνιση της σελίδας με σκούρα χρώματα"),
    ("App\nTheme", "Θέμα\nεφαρμογής"),
    ("Colours for the ribbon and menus (also under File ▸ Theme)", "Χρώματα για την κορδέλα και τα μενού (και στο Αρχείο ▸ Θέμα)"),
    ("Show", "Εμφάνιση"),
    ("Ruler", "Χάρακας"),
    ("Gridlines", "Γραμμές πλέγματος"),
    ("Navigation Pane", "Παράθυρο περιήγησης"),
    ("Zoom", "Ζουμ"),
    ("One Page", "Μία σελίδα"),
    ("Multiple Pages", "Πολλές σελίδες"),
    ("Page Width", "Πλάτος σελίδας"),
    ("Window", "Παράθυρο"),
    ("New Window", "Νέο παράθυρο"),
    ("Arrange All", "Τακτοποίηση όλων"),
    ("Split", "Διαίρεση"),
    ("Switch\nWindows", "Εναλλαγή\nπαραθύρων"),
    ("Macros", "Μακροεντολές"),
    // Help
    ("Contact\nSupport", "Επικοινωνία με\nυποστήριξη"),
    ("Show\nTraining", "Εμφάνιση\nεκπαίδευσης"),
    // Status bar
    ("Customize Status Bar", "Προσαρμογή γραμμής κατάστασης"),
    ("Page Number", "Αριθμός σελίδας"),
    ("Accessibility Checker", "Έλεγχος προσβασιμότητας"),
    ("View Shortcuts", "Συντομεύσεις προβολής"),
    ("Zoom Slider", "Ρυθμιστικό ζουμ"),
    ("Page number", "Αριθμός σελίδας"),
    ("Word count", "Καταμέτρηση λέξεων"),
    ("View shortcuts", "Συντομεύσεις προβολής"),
    ("Zoom slider", "Ρυθμιστικό ζουμ"),
    ("Hide the status bar", "Απόκρυψη της γραμμής κατάστασης"),
    ("Page {} of {}", "Σελίδα {} από {}"),
    ("{} words", "{} λέξεις"),
    ("1 word", "1 λέξη"),
    ("English (United States)", "Αγγλικά (Ηνωμένες Πολιτείες)"),
    ("Accessibility: Good to go", "Προσβασιμότητα: Όλα εντάξει"),
    ("Focus", "Εστίαση"),
    // Navigation pane, header/footer labels
    ("Navigation", "Περιήγηση"),
    ("Search document", "Αναζήτηση στο έγγραφο"),
    ("Create an interactive outline of your document by applying Heading styles.", "Δημιουργήστε ένα αλληλεπιδραστικό διάγραμμα του εγγράφου σας εφαρμόζοντας στυλ επικεφαλίδων."),
    // File (backstage)
    ("Back", "Πίσω"),
    ("file\u{4}Home", "Αρχική"),
    ("New", "Δημιουργία"),
    ("Open", "Άνοιγμα"),
    ("Info", "Πληροφορίες"),
    ("Theme", "Θέμα"),
    ("Options", "Επιλογές"),
    ("Save", "Αποθήκευση"),
    ("Save As", "Αποθήκευση ως"),
    ("Print", "Εκτύπωση"),
    ("Export", "Εξαγωγή"),
    ("Close", "Κλείσιμο"),
    ("Good day", "Καλημέρα"),
    ("Blank document", "Κενό έγγραφο"),
    ("Browse…", "Αναζήτηση…"),
    ("Not saved yet", "Δεν έχει αποθηκευτεί ακόμα"),
    ("Words", "Λέξεις"),
    ("Parts in package", "Τμήματα στο πακέτο"),
    ("Compatibility mode", "Λειτουργία συμβατότητας"),
    ("Mode 15 (current)", "Λειτουργία 15 (τρέχουσα)"),
    ("Mode 14", "Λειτουργία 14"),
    ("Mode 12", "Λειτουργία 12"),
    ("Legacy ({})", "Παλαιού τύπου ({})"),
    ("Recent", "Πρόσφατα"),
    ("Clear all", "Απαλοιφή όλων"),
    ("Remove every document from the Recent list", "Κατάργηση όλων των εγγράφων από τη λίστα πρόσφατων"),
    ("Documents you open will show up here.", "Τα έγγραφα που ανοίγετε θα εμφανίζονται εδώ."),
    ("Remove from list", "Κατάργηση από τη λίστα"),
    ("Copy path", "Αντιγραφή διαδρομής"),
    ("Clear all recent documents", "Απαλοιφή όλων των πρόσφατων εγγράφων"),
    // File ▸ Theme
    ("Colours for the ribbon, menus and the area around the pages. Your documents are not changed.", "Χρώματα για την κορδέλα, τα μενού και την περιοχή γύρω από τις σελίδες. Τα έγγραφά σας δεν αλλάζουν."),
    (
        "Following your system's appearance: light mode uses {}, dark mode uses {}. Picking a theme replaces the one for its mode.",
        "Ακολουθείται η εμφάνιση του συστήματος: η φωτεινή λειτουργία χρησιμοποιεί το {}, η σκούρα το {}. Η επιλογή ενός θέματος αντικαθιστά εκείνο της λειτουργίας του.",
    ),
    ("dark", "σκούρο"),
    ("light", "φωτεινό"),
    ("Black", "Μαύρο"),
    ("White", "Λευκό"),
    // File ▸ Options
    ("Changes apply right away and are remembered.", "Οι αλλαγές εφαρμόζονται αμέσως και διατηρούνται."),
    ("Appearance", "Εμφάνιση"),
    ("Interface language", "Γλώσσα περιβάλλοντος εργασίας"),
    ("Same as the system ({})", "Ίδια με του συστήματος ({})"),
    ("Use one theme", "Χρήση ενός θέματος"),
    ("Match the system's light or dark mode", "Αντιστοίχιση με τη φωτεινή ή σκούρα λειτουργία του συστήματος"),
    ("Light mode", "Φωτεινή λειτουργία"),
    ("Dark mode", "Σκούρα λειτουργία"),
    ("Your system doesn't report light or dark mode, so the single theme above is used.", "Το σύστημά σας δεν αναφέρει φωτεινή ή σκούρα λειτουργία, επομένως χρησιμοποιείται το παραπάνω θέμα."),
    ("Accent colour", "Χρώμα έμφασης"),
    ("The theme's own accent", "Το χρώμα έμφασης του θέματος"),
    ("Any colour", "Οποιοδήποτε χρώμα"),
    ("Interface size", "Μέγεθος περιβάλλοντος εργασίας"),
    ("Ribbon and status bar", "Κορδέλα και γραμμή κατάστασης"),
    ("Collapse the ribbon to its tabs (Ctrl+F1, or double-click a tab)", "Σύμπτυξη της κορδέλας στις καρτέλες της (Ctrl+F1 ή διπλό κλικ σε καρτέλα)"),
    ("Show the status bar", "Εμφάνιση της γραμμής κατάστασης"),
    ("Tip: right-click the status bar to change these too.", "Συμβουλή: κάντε δεξί κλικ στη γραμμή κατάστασης για να τα αλλάξετε κι από εκεί."),
    ("Blinking cursor", "Δρομέας που αναβοσβήνει"),
    ("Cursor width", "Πλάτος δρομέα"),
    ("Zoom when starting", "Ζουμ κατά την εκκίνηση"),
    ("Show the ruler", "Εμφάνιση του χάρακα"),
    ("Show the navigation pane", "Εμφάνιση του παραθύρου περιήγησης"),
    ("Restore all defaults", "Επαναφορά όλων των προεπιλογών"),
    // Accent colour names
    ("Blue", "Μπλε"),
    ("Sky", "Γαλάζιο"),
    ("Teal", "Γαλαζοπράσινο"),
    ("Green", "Πράσινο"),
    ("Olive", "Λαδί"),
    ("Orange", "Πορτοκαλί"),
    ("Red", "Κόκκινο"),
    ("Pink", "Ροζ"),
    ("Purple", "Μωβ"),
    ("Violet", "Βιολετί"),
    ("Slate", "Σχιστόλιθος"),
    // Dialogs
    ("OK", "OK"),
    ("Cancel", "Άκυρο"),
    ("Don't Save", "Χωρίς αποθήκευση"),
    ("Do you want to save changes to “{}”?", "Θέλετε να αποθηκεύσετε τις αλλαγές στο «{}»;"),
    ("Characters (no spaces)", "Χαρακτήρες (χωρίς διαστήματα)"),
    ("Characters (with spaces)", "Χαρακτήρες (με διαστήματα)"),
    ("Paragraphs", "Παράγραφοι"),
    ("Find and Replace", "Εύρεση και αντικατάσταση"),
    ("Find what:", "Εύρεση του:"),
    ("Replace with:", "Αντικατάσταση με:"),
    ("Find Next", "Εύρεση επόμενου"),
    ("Replace All", "Αντικατάσταση όλων"),
    ("RustyDocumentWriter made {} replacement(s).", "Το RustyDocumentWriter έκανε {} αντικαταστάσεις."),
    ("About RustyDocumentWriter", "Πληροφορίες για το RustyDocumentWriter"),
    (
        "RustyDocumentWriter — an open-source word processor built around faithful DOCX layout.\n\nWritten in Rust. Layout follows the format's real-world rules (GDI line metrics, tab, spacing, widow/orphan and table conventions) and documents round-trip without losing unknown content.",
        "RustyDocumentWriter — ένας επεξεργαστής κειμένου ανοιχτού κώδικα, χτισμένος γύρω από την πιστή διάταξη των αρχείων DOCX.\n\nΓραμμένος σε Rust. Η διάταξη ακολουθεί τους πραγματικούς κανόνες της μορφής (μετρικές γραμμών GDI, στηλοθέτες, αποστάσεις, χήρες/ορφανές γραμμές και συμβάσεις πινάκων) και τα έγγραφα αποθηκεύονται ξανά χωρίς να χάνεται άγνωστο περιεχόμενο.",
    ),
    ("Not available yet", "Δεν είναι ακόμα διαθέσιμο"),
    (
        "“{}” isn't implemented in this version of RustyDocumentWriter yet. Any existing content of this kind in your documents is preserved when you save.",
        "Η δυνατότητα «{}» δεν υπάρχει ακόμα σε αυτήν την έκδοση του RustyDocumentWriter. Όποιο υπάρχον περιεχόμενο αυτού του είδους έχουν τα έγγραφά σας διατηρείται κατά την αποθήκευση.",
    ),
    ("Macros were removed", "Οι μακροεντολές καταργήθηκαν"),
    (
        "This type of file can't contain macros, so they were left out. To keep them, save as a Word Macro-Enabled Document (.docm).",
        "Αυτός ο τύπος αρχείου δεν μπορεί να περιέχει μακροεντολές, γι' αυτό παραλείφθηκαν. Για να τις διατηρήσετε, αποθηκεύστε ως Έγγραφο του Word με δυνατότητα μακροεντολών (.docm).",
    ),
    ("Couldn't save the document", "Δεν ήταν δυνατή η αποθήκευση του εγγράφου"),
    ("Couldn't open the document", "Δεν ήταν δυνατό το άνοιγμα του εγγράφου"),
    ("Couldn't export", "Δεν ήταν δυνατή η εξαγωγή"),
    ("Couldn't read the picture", "Δεν ήταν δυνατή η ανάγνωση της εικόνας"),
    ("Unsupported picture", "Μη υποστηριζόμενη εικόνα"),
    ("Something went wrong", "Κάτι πήγε στραβά"),
    ("The conversion stopped unexpectedly.", "Η μετατροπή σταμάτησε απροσδόκητα."),
    ("LibreOffice is needed", "Χρειάζεται το LibreOffice"),
    (
        "“{}” is a {}. RustyDocumentWriter converts these with LibreOffice, which isn't installed. Install it from libreoffice.org, or save the file as .docx in another program.",
        "Το «{}» είναι {}. Το RustyDocumentWriter μετατρέπει αυτά τα αρχεία με το LibreOffice, το οποίο δεν είναι εγκατεστημένο. Εγκαταστήστε το από το libreoffice.org ή αποθηκεύστε το αρχείο ως .docx σε άλλο πρόγραμμα.",
    ),
    ("Converting “{}”…", "Μετατροπή του «{}»…"),
    ("Exporting “{}”…", "Εξαγωγή του «{}»…"),
    ("Exporting “{}” with LibreOffice…", "Εξαγωγή του «{}» με το LibreOffice…"),
    ("Cannot write {}: {}", "Δεν είναι δυνατή η εγγραφή του {}: {}"),
    ("RustyDocumentWriter can't export .{} files.", "Το RustyDocumentWriter δεν μπορεί να εξαγάγει αρχεία .{}."),
    ("Converted a {}", "Μετατράπηκε: {}"),
    (
        "“{}” was converted to a Word document with LibreOffice. Saving creates a new .docx; the original file is not changed.",
        "Το «{}» μετατράπηκε σε έγγραφο του Word με το LibreOffice. Η αποθήκευση δημιουργεί νέο αρχείο .docx· το αρχικό αρχείο δεν αλλάζει.",
    ),
    // File formats (convert::Format::describe)
    ("Word document", "έγγραφο του Word"),
    ("Word 97-2003 document (.doc)", "έγγραφο του Word 97-2003 (.doc)"),
    ("password-protected document", "έγγραφο με κωδικό πρόσβασης"),
    ("OpenDocument text (.odt)", "κείμενο OpenDocument (.odt)"),
    ("Rich Text Format (.rtf)", "αρχείο εμπλουτισμένου κειμένου (.rtf)"),
    ("web page (.html)", "ιστοσελίδα (.html)"),
    ("plain text", "απλό κείμενο"),
    ("unrecognised file", "μη αναγνωρίσιμο αρχείο"),
    // File dialog filters
    ("All Documents", "Όλα τα έγγραφα"),
    ("Word Documents", "Έγγραφα του Word"),
    ("Other Documents (converted with LibreOffice)", "Άλλα έγγραφα (μετατροπή με το LibreOffice)"),
    ("Word Document", "Έγγραφο του Word"),
    ("Word Macro-Enabled Document", "Έγγραφο του Word με δυνατότητα μακροεντολών"),
    ("Word Template", "Πρότυπο του Word"),
    ("Word Macro-Enabled Template", "Πρότυπο του Word με δυνατότητα μακροεντολών"),
    ("Plain Text", "Απλό κείμενο"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Run `f` with Greek active, then switch back (tests share the global).
    fn in_greek<R>(f: impl FnOnce() -> R) -> R {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set(Lang::El);
        let r = f();
        set(Lang::En);
        r
    }

    #[test]
    fn lookup_and_templates() {
        assert_eq!(tr("Paste"), "Paste");
        assert_eq!(tr("file\u{4}Home"), "Home");
        assert_eq!(tf("Page {} of {}", &[&2, &7]), "Page 2 of 7");
        assert_eq!(fill("{1} before {0}, {}", &[&"a", &"b"]), "b before a, a");
        assert_eq!(fill("braces {x} stay", &[&1]), "braces {x} stay");
        in_greek(|| {
            assert_eq!(tr("Paste"), "Επικόλληση");
            assert_eq!(tr("file\u{4}Home"), "Αρχική");
            assert_eq!(tr("Home"), "Κεντρική");
            assert_eq!(tr("not translated"), "not translated");
            assert_eq!(tf("Page {} of {}", &[&2, &7]), "Σελίδα 2 από 7");
        });
    }

    #[test]
    fn table_is_consistent() {
        let mut seen = HashSet::new();
        for (en, el) in EL {
            assert!(seen.insert(*en), "duplicate key {en:?}");
            assert!(!el.is_empty(), "empty translation for {en:?}");
            let holes = |s: &str| s.matches("{}").count() + (0..9).map(|i| s.matches(&format!("{{{i}}}")).count()).sum::<usize>();
            assert_eq!(holes(en), holes(el), "placeholders differ for {en:?}");
            // Ribbon labels are drawn line by line; keep them to two lines.
            if en.len() < 40 {
                assert!(el.lines().count() <= 2, "too many lines in {el:?}");
            }
        }
    }

    /// The library's display names (shapes, themes, styles…) have Greek names.
    #[test]
    fn library_names_are_translated() {
        let names = rusty_document_writer::shapes::SHAPES.iter().map(|s| s.name)
            .chain(rusty_document_writer::texttheme::THEMES.iter().map(|t| t.name))
            .chain(rusty_document_writer::wordart::STYLES.iter().map(|s| s.name))
            .chain(rusty_document_writer::pagenum::STYLES.iter().map(|s| s.name))
            .chain(super::super::settings::ACCENTS.iter().map(|(n, _)| *n));
        for n in names {
            assert!(greek().contains_key(n), "no Greek for {n:?}");
        }
    }

    /// Every interface string in `src/app` is either translated or deliberately left
    /// alone (ids, file extensions, product names…). New UI text has to go somewhere.
    #[test]
    fn app_strings_are_covered() {
        let keep: HashSet<&str> = KEEP.iter().copied().collect();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app");
        let mut missing = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") || path.file_name().is_some_and(|n| n == "i18n.rs" || n == "icons.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let src = src.split("#[cfg(test)]").next().unwrap();
            for lit in string_literals(src) {
                if looks_like_ui_text(&lit) && !greek().contains_key(lit.as_str()) && !keep.contains(lit.as_str()) {
                    missing.push(format!("{}: {lit:?}", path.file_name().unwrap().to_string_lossy()));
                }
            }
        }
        assert!(missing.is_empty(), "untranslated interface strings (add them to EL or KEEP):\n{}", missing.join("\n"));
    }

    /// Strings that look like UI text but stay as they are.
    const KEEP: &[&str] = &[
        "RustyDocumentWriter", "Word", "VS Code", "Zed", "SmartArt", "WordArt", "PDF", "Arial", "Aa", "Abc", "Aʙᴄ", "ABC", "R",
        "Normal", "NoSpacing", "Heading1", "Heading2", "Hyperlink", "Title", "Subtitle", "Quote", "100%", "OK",
        "__quit", "{}{} - RustyDocumentWriter", "# RustyDocumentWriter settings (File > Options)\n", "WordArt rPr",
        // Paper sizes keep their (US) names in Greek Word too.
        "Letter  8.5\" x 11\"", "Legal  8.5\" x 14\"", "Executive  7.25\" x 10.5\"",
    ];

    fn looks_like_ui_text(s: &str) -> bool {
        // An English word with a capital, or several lower-case words: "Page {} of {}", "lowercase".
        let has_word = s.as_bytes().windows(2).any(|w| w[0].is_ascii_uppercase() && w[1].is_ascii_lowercase());
        let word = !s.contains(' ');
        let camel = word && s.starts_with(|c: char| c.is_ascii_lowercase());
        let is_id = camel || word && s.contains([':', '/', '.', '\\']) || s.contains('<');
        let palette = super::super::theme::PALETTES.iter().any(|p| p.name == s);
        has_word && !is_id && !palette
    }

    /// The (unescaped) string literals of some Rust source, skipping comments.
    fn string_literals(src: &str) -> Vec<String> {
        let mut out = Vec::new();
        let b: Vec<char> = src.chars().collect();
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                '/' if b.get(i + 1) == Some(&'/') => {
                    while i < b.len() && b[i] != '\n' {
                        i += 1;
                    }
                }
                '\'' => {
                    // Char literal ('"', '\'', '\u{..}') or a lifetime.
                    if b.get(i + 1) == Some(&'\\') {
                        i += 2;
                        while i < b.len() && b[i] != '\'' {
                            i += 1;
                        }
                        i += 1;
                    } else if b.get(i + 2) == Some(&'\'') {
                        i += 3;
                    } else {
                        i += 1;
                    }
                }
                'r' if b.get(i + 1) == Some(&'#') && (i == 0 || !b[i - 1].is_alphanumeric()) => {
                    // r#"..."# (no UI text in raw strings): skip it.
                    let end: String = "\"#".into();
                    let rest: String = b[i..].iter().collect();
                    i += rest.find(&end).map(|n| rest[..n].chars().count() + 2).unwrap_or(rest.chars().count());
                }
                '"' => {
                    let mut s = String::new();
                    i += 1;
                    while i < b.len() && b[i] != '"' {
                        if b[i] == '\\' {
                            i += 1;
                            match b.get(i) {
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some('u') => {
                                    let close = (i..b.len()).find(|&j| b[j] == '}').unwrap();
                                    let hex: String = b[i + 2..close].iter().collect();
                                    s.extend(char::from_u32(u32::from_str_radix(&hex, 16).unwrap()));
                                    i = close;
                                }
                                Some(c) => s.push(*c),
                                None => {}
                            }
                        } else {
                            s.push(b[i]);
                        }
                        i += 1;
                    }
                    out.push(s);
                    i += 1;
                }
                _ => i += 1,
            }
        }
        out
    }
}
