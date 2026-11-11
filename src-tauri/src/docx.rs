//! Génération d'un .docx propre (OOXML) : page A4, titre, tableau
//! d'informations, paragraphes justifiés, hébreu en RTL, pied de page
//! avec numérotation. Aucune dépendance à Word pour fabriquer le fichier.

use crate::jobs::Job;
use crate::models;
use crate::text;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;

const CONTENT_WIDTH: i32 = 9070;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn run_opts(
    text: &str,
    rtl: bool,
    bold: bool,
    color: Option<&str>,
    size_half_points: Option<u32>,
    font: Option<&str>,
) -> String {
    let mut rpr = String::from("<w:rPr>");
    if let Some(font) = font {
        rpr.push_str(&format!(
            "<w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\" w:cs=\"{font}\"/>"
        ));
    }
    if bold {
        rpr.push_str("<w:b/><w:bCs/>");
    }
    if let Some(color) = color {
        rpr.push_str(&format!("<w:color w:val=\"{color}\"/>"));
    }
    if let Some(size) = size_half_points {
        rpr.push_str(&format!("<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>"));
    }
    if rtl {
        rpr.push_str("<w:rtl/><w:lang w:bidi=\"he-IL\"/>");
    } else {
        rpr.push_str("<w:rtl w:val=\"0\"/>");
    }
    rpr.push_str("</w:rPr>");
    format!(
        "<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
        esc(text)
    )
}

fn paragraph(style: Option<&str>, extra_ppr: &str, runs: &str) -> String {
    let mut ppr = String::from("<w:pPr>");
    if let Some(style) = style {
        ppr.push_str(&format!("<w:pStyle w:val=\"{style}\"/>"));
    }
    ppr.push_str(extra_ppr);
    ppr.push_str("</w:pPr>");
    format!("<w:p>{ppr}{runs}</w:p>")
}

pub fn write(job: &Job, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let has_rtl = job.paragraphs.iter().any(|p| text::is_rtl(&p.text));
    let title = text::display_title(&job.name);
    let title_rtl = text::is_rtl(&title);
    let date = text::date_fr();
    let duration = text::format_duration(job.duration);
    let lang = text::language_name(&job.language);
    let model_name = models::find(&job.model)
        .map(|m| m.name)
        .unwrap_or(job.model.as_str());
    let words = job.words;

    let mut body = String::new();

    // Titre
    let title_style = if title_rtl { "TitleRtl" } else { "Title" };
    body.push_str(&paragraph(
        Some(title_style),
        "",
        &run_opts(
            &title,
            title_rtl,
            true,
            Some("241F1A"),
            None,
            Some(if title_rtl { "Times New Roman" } else { "Georgia" }),
        ),
    ));

    // Sous-titre + filet
    body.push_str(&paragraph(
        Some("Subtitle"),
        "",
        &run_opts(
            &format!("Transcription · {date}"),
            false,
            false,
            Some("8A7B6C"),
            None,
            None,
        ),
    ));
    body.push_str(&paragraph(
        None,
        "<w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"6\" w:color=\"E0D4C4\"/></w:pBdr><w:spacing w:before=\"0\" w:after=\"260\" w:line=\"240\" w:lineRule=\"auto\"/>",
        "",
    ));

    // Tableau d'informations
    let rows: [(&str, String); 5] = [
        ("Fichier source", job.name.clone()),
        ("Durée", duration),
        ("Langue", lang.to_string()),
        ("Modèle", model_name.to_string()),
        ("Mots", words.to_string()),
    ];
    let mut table = String::from(
        "<w:tbl><w:tblPr><w:tblW w:w=\"9070\" w:type=\"dxa\"/>\
         <w:tblBorders><w:top w:val=\"none\" w:sz=\"0\" w:space=\"0\" w:color=\"auto\"/>\
         <w:left w:val=\"none\" w:sz=\"0\" w:space=\"0\" w:color=\"auto\"/>\
         <w:bottom w:val=\"none\" w:sz=\"0\" w:space=\"0\" w:color=\"auto\"/>\
         <w:right w:val=\"none\" w:sz=\"0\" w:space=\"0\" w:color=\"auto\"/>\
         <w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"EBE1D4\"/>\
         <w:insideV w:val=\"none\" w:sz=\"0\" w:space=\"0\" w:color=\"auto\"/></w:tblBorders>\
         <w:tblLayout w:type=\"fixed\"/>\
         <w:tblCellMar><w:top w:w=\"70\" w:type=\"dxa\"/><w:left w:w=\"0\" w:type=\"dxa\"/>\
         <w:bottom w:w=\"70\" w:type=\"dxa\"/><w:right w:w=\"120\" w:type=\"dxa\"/></w:tblCellMar>\
         <w:tblLook w:val=\"04A0\" w:firstRow=\"1\" w:lastRow=\"0\" w:firstColumn=\"1\" w:lastColumn=\"0\" w:noHBand=\"0\" w:noVBand=\"1\"/>\
         </w:tblPr><w:tblGrid><w:gridCol w:w=\"2150\"/><w:gridCol w:w=\"6920\"/></w:tblGrid>",
    );
    for (label, value) in rows {
        let value_rtl = text::is_rtl(&value);
        table.push_str(
            "<w:tr><w:trPr><w:cantSplit/></w:trPr>\
             <w:tc><w:tcPr><w:tcW w:w=\"2150\" w:type=\"dxa\"/><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"FBF7F0\"/></w:tcPr>",
        );
        table.push_str(&paragraph(
            Some("MetaLabel"),
            "",
            &run_opts(label, false, true, Some("938474"), Some(15), None),
        ));
        table.push_str("</w:tc><w:tc><w:tcPr><w:tcW w:w=\"6920\" w:type=\"dxa\"/></w:tcPr>");
        table.push_str(&paragraph(
            Some("MetaValue"),
            "",
            &run_opts(
                &value,
                value_rtl,
                false,
                Some("3A342E"),
                Some(18),
                if value_rtl { Some("Times New Roman") } else { None },
            ),
        ));
        table.push_str("</w:tc></w:tr>");
    }
    table.push_str("</w:tbl>");
    body.push_str(&table);
    body.push_str(&paragraph(
        None,
        "<w:spacing w:before=\"320\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/>",
        "",
    ));

    // Titre de section
    body.push_str(&paragraph(
        Some("Heading"),
        "",
        &run_opts("Transcription", false, true, Some("B4552D"), None, None),
    ));

    // Corps
    for p in &job.paragraphs {
        let rtl = text::is_rtl(&p.text);
        let style = if rtl { "BodyRtl" } else { "Body" };
        let stamp = if job.timestamps {
            run_opts(
                &format!("  [{}]", text::format_timestamp(p.start)),
                false,
                true,
                Some("B4552D"),
                Some(17),
                Some("Calibri"),
            )
        } else {
            String::new()
        };
        let body_run = run_opts(
            &p.text,
            rtl,
            false,
            Some("241F1A"),
            Some(if rtl { 24 } else { 22 }),
            Some(if rtl { "Times New Roman" } else { "Calibri" }),
        );
        // En RTL, l'horodatage est placé en fin logique : il apparaît à
        // gauche, là où l'œil termine la ligne hébraïque.
        let runs = if rtl {
            format!("{body_run}{stamp}")
        } else {
            format!("{stamp}{body_run}")
        };
        body.push_str(&paragraph(Some(style), "", &runs));
    }

    if job.paragraphs.is_empty() {
        body.push_str(&paragraph(
            Some("Body"),
            "",
            &run_opts(
                "Aucune parole détectée dans ce fichier.",
                false,
                false,
                Some("8A7B6C"),
                None,
                None,
            ),
        ));
    }

    let _ = has_rtl;

    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
         <w:body>{body}\
         <w:sectPr><w:footerReference w:type=\"default\" r:id=\"rId1\"/>\
         <w:pgSz w:w=\"11906\" w:h=\"16838\"/>\
         <w:pgMar w:top=\"1418\" w:right=\"1418\" w:bottom=\"1418\" w:left=\"1418\" \
         w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/>\
         <w:cols w:space=\"708\"/><w:docGrid w:linePitch=\"360\"/></w:sectPr>\
         </w:body></w:document>"
    );

    let styles = styles_xml();
    let footer = footer_xml();
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let core = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" \
         xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
         <dc:title>{}</dc:title><dc:creator>Plume</dc:creator><cp:lastModifiedBy>Plume</cp:lastModifiedBy>\
         <dc:description>Transcription locale réalisée avec Whisper.</dc:description>\
         <dcterms:created xsi:type=\"dcterms:W3CDTF\">{}</dcterms:created>\
         <dcterms:modified xsi:type=\"dcterms:W3CDTF\">{}</dcterms:modified>\
         </cp:coreProperties>",
        esc(&title),
        now,
        now
    );
    let app = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
        <Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" \
        xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">\
        <Application>Plume</Application><AppVersion>0.1.0</AppVersion>\
        <Company>Plume</Company><DocSecurity>0</DocSecurity></Properties>";

    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
        <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
        <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
        <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
        <Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
        <Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
        <Override PartName=\"/word/settings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml\"/>\
        <Override PartName=\"/word/footer1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml\"/>\
        <Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
        <Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>\
        </Types>";

    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
        <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
        <Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/>\
        </Relationships>";

    let doc_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer\" Target=\"footer1.xml\"/>\
        <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
        <Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings\" Target=\"settings.xml\"/>\
        </Relationships>";

    let settings = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
        <w:settings xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
        <w:zoom w:percent=\"100\"/><w:defaultTabStop w:val=\"708\"/>\
        <w:characterSpacingControl w:val=\"doNotCompress\"/>\
        <w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat>\
        </w:settings>";

    let file = std::fs::File::create(path)
        .with_context(|| format!("Création du fichier Word impossible : {}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let entries: [(&str, &str); 8] = [
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", rels),
        ("docProps/core.xml", &core),
        ("docProps/app.xml", app),
        ("word/document.xml", &document),
        ("word/styles.xml", &styles),
        ("word/footer1.xml", &footer),
        ("word/_rels/document.xml.rels", doc_rels),
    ];
    for (name, content) in entries {
        zip.start_file(name, opts).context("Archive Word invalide")?;
        zip.write_all(content.as_bytes())
            .context("Écriture du document impossible")?;
    }
    zip.start_file("word/settings.xml", opts)
        .context("Archive Word invalide")?;
    zip.write_all(settings.as_bytes())
        .context("Écriture du document impossible")?;
    zip.finish().context("Finalisation du document impossible")?;
    Ok(())
}

fn footer_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <w:ftr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
         <w:p><w:pPr><w:pStyle w:val=\"Footer\"/>\
         <w:tabs><w:tab w:val=\"right\" w:pos=\"{CONTENT_WIDTH}\"/></w:tabs>\
         <w:spacing w:before=\"0\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
         <w:r><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/><w:color w:val=\"A79A8B\"/><w:sz w:val=\"16\"/></w:rPr>\
         <w:t xml:space=\"preserve\">Plume · transcription locale · </w:t></w:r>\
         <w:r><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/><w:color w:val=\"A79A8B\"/><w:sz w:val=\"16\"/></w:rPr><w:t>page </w:t></w:r>\
         <w:fldSimple w:instr=\" PAGE \"><w:r><w:rPr><w:color w:val=\"A79A8B\"/><w:sz w:val=\"16\"/></w:rPr><w:t>1</w:t></w:r></w:fldSimple>\
         <w:r><w:rPr><w:color w:val=\"A79A8B\"/><w:sz w:val=\"16\"/></w:rPr><w:t xml:space=\"preserve\"> / </w:t></w:r>\
         <w:fldSimple w:instr=\" NUMPAGES \"><w:r><w:rPr><w:color w:val=\"A79A8B\"/><w:sz w:val=\"16\"/></w:rPr><w:t>1</w:t></w:r></w:fldSimple>\
         </w:p></w:ftr>"
    )
}

fn styles_xml() -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
         <w:docDefaults><w:rPrDefault><w:rPr>\
         <w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:cs=\"Times New Roman\"/>\
         <w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/>\
         <w:lang w:val=\"fr-FR\" w:eastAsia=\"fr-FR\" w:bidi=\"he-IL\"/>\
         </w:rPr></w:rPrDefault>\
         <w:pPrDefault><w:pPr><w:spacing w:after=\"120\" w:line=\"300\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault>\
         </w:docDefaults>\
         <w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>",
    );

    s.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Titre Plume\"/>\
         <w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/>\
         <w:pPr><w:spacing w:before=\"0\" w:after=\"60\" w:line=\"264\" w:lineRule=\"auto\"/></w:pPr>\
         <w:rPr><w:rFonts w:ascii=\"Georgia\" w:hAnsi=\"Georgia\"/><w:b/><w:color w:val=\"241F1A\"/>\
         <w:sz w:val=\"52\"/><w:szCs w:val=\"52\"/></w:rPr></w:style>",
    );
    s.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"TitleRtl\"><w:name w:val=\"Titre Plume RTL\"/>\
         <w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/>\
         <w:pPr><w:bidi/><w:jc w:val=\"right\"/><w:spacing w:before=\"0\" w:after=\"60\" w:line=\"288\" w:lineRule=\"auto\"/></w:pPr>\
         <w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\" w:cs=\"Times New Roman\"/>\
         <w:rtl/><w:b/><w:color w:val=\"241F1A\"/><w:sz w:val=\"52\"/><w:szCs w:val=\"52\"/>\
         <w:lang w:bidi=\"he-IL\"/></w:rPr></w:style>",
    );
    s.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"Subtitle\"><w:name w:val=\"Sous-titre Plume\"/>\
         <w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/>\
         <w:pPr><w:spacing w:before=\"0\" w:after=\"120\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
         <w:rPr><w:color w:val=\"8A7B6C\"/><w:sz w:val=\"19\"/><w:szCs w:val=\"19\"/></w:rPr></w:style>",
    );
    s.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"Heading\"><w:name w:val=\"Titre de section\"/>\
         <w:basedOn w:val=\"Normal\"/><w:next w:val=\"Body\"/><w:qFormat/>\
         <w:pPr><w:keepNext/><w:spacing w:before=\"240\" w:after=\"180\" w:line=\"276\" w:lineRule=\"auto\"/></w:pPr>\
         <w:rPr><w:rFonts w:ascii=\"Georgia\" w:hAnsi=\"Georgia\"/><w:b/><w:color w:val=\"B4552D\"/>\
         <w:sz w:val=\"28\"/><w:szCs w:val=\"28\"/></w:rPr></w:style>",
    );
    s.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"Body\"><w:name w:val=\"Corps Plume\"/>\
         <w:basedOn w:val=\"Normal\"/><w:qFormat/>\
         <w:pPr><w:spacing w:before=\"0\" w:after=\"160\" w:line=\"360\" w:lineRule=\"auto\"/><w:jc w:val=\"both\"/></w:pPr>\
         <w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/>\
         <w:color w:val=\"241F1A\"/></w:rPr></w:style>",
    );
    s.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"BodyRtl\"><w:name w:val=\"Corps Plume RTL\"/>\
         <w:basedOn w:val=\"Normal\"/><w:qFormat/>\
         <w:pPr><w:bidi/><w:spacing w:before=\"0\" w:after=\"160\" w:line=\"360\" w:lineRule=\"auto\"/><w:jc w:val=\"both\"/></w:pPr>\
         <w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\" w:cs=\"Times New Roman\"/>\
         <w:rtl/><w:sz w:val=\"24\"/><w:szCs w:val=\"24\"/><w:color w:val=\"241F1A\"/>\
         <w:lang w:bidi=\"he-IL\"/></w:rPr></w:style>",
    );
    s.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"MetaLabel\"><w:name w:val=\"Étiquette\"/>\
         <w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
         <w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/><w:b/><w:color w:val=\"938474\"/>\
         <w:sz w:val=\"15\"/><w:szCs w:val=\"15\"/></w:rPr></w:style>",
    );
    s.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"MetaValue\"><w:name w:val=\"Valeur\"/>\
         <w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
         <w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/><w:color w:val=\"3A342E\"/>\
         <w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/></w:rPr></w:style>",
    );
    s.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"Footer\"><w:name w:val=\"Pied de page Plume\"/>\
         <w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
         <w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/><w:color w:val=\"A79A8B\"/><w:sz w:val=\"16\"/></w:rPr></w:style>",
    );
    s.push_str("</w:styles>");
    s
}
