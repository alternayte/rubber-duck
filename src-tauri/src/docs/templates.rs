use super::model::{BuiltinTemplate, TemplateSection};

const PRD_TEMPLATE: &str = include_str!("../../templates/prd.md");
const SDD_TEMPLATE: &str = include_str!("../../templates/sdd.md");
const TEST_PLAN_TEMPLATE: &str = include_str!("../../templates/test-plan.md");
const ADR_TEMPLATE: &str = include_str!("../../templates/adr.md");

pub fn get_builtin_templates() -> Vec<BuiltinTemplate> {
    vec![
        BuiltinTemplate {
            name: "PRD".to_string(),
            content: PRD_TEMPLATE.to_string(),
        },
        BuiltinTemplate {
            name: "SDD".to_string(),
            content: SDD_TEMPLATE.to_string(),
        },
        BuiltinTemplate {
            name: "Test Plan".to_string(),
            content: TEST_PLAN_TEMPLATE.to_string(),
        },
        BuiltinTemplate {
            name: "ADR".to_string(),
            content: ADR_TEMPLATE.to_string(),
        },
    ]
}

/// Parse a template's markdown content into sections.
///
/// Looks for paired `<!-- section: X -->` and `<!-- directive: Y -->` comments.
/// Sections without a corresponding directive are skipped.
/// The directive may span multiple lines — everything from `<!-- directive:` to
/// the closing `-->` is captured as the directive text.
pub fn parse_template(content: &str) -> Vec<TemplateSection> {
    let mut sections: Vec<TemplateSection> = Vec::new();
    let mut current_section: Option<String> = None;
    let mut current_directive: Option<String> = None;
    let mut in_directive = false;
    let mut directive_buf = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect opening of a directive block (may close on same line)
        if trimmed.starts_with("<!-- directive:") {
            if let Some(close) = trimmed.find("-->") {
                // Single-line directive: <!-- directive: ... -->
                let inner = trimmed["<!-- directive:".len()..close].trim();
                current_directive = Some(inner.to_string());
                in_directive = false;
                directive_buf.clear();
            } else {
                // Multi-line directive: <!-- directive:\n...\n-->
                in_directive = true;
                directive_buf.clear();
                let inner = trimmed["<!-- directive:".len()..].trim();
                if !inner.is_empty() {
                    directive_buf.push_str(inner);
                    directive_buf.push(' ');
                }
            }
            continue;
        }

        if in_directive {
            if trimmed.ends_with("-->") {
                let inner = trimmed.trim_end_matches("-->").trim();
                if !inner.is_empty() {
                    directive_buf.push_str(inner);
                    directive_buf.push(' ');
                }
                current_directive = Some(directive_buf.trim().to_string());
                directive_buf.clear();
                in_directive = false;
            } else {
                directive_buf.push_str(trimmed);
                directive_buf.push(' ');
            }
            continue;
        }

        // Detect section marker: <!-- section: Name -->
        if trimmed.starts_with("<!-- section:") {
            if let Some(close) = trimmed.find("-->") {
                let name = trimmed["<!-- section:".len()..close].trim().to_string();

                // If we have a pending section + directive pair, save it
                if let (Some(sec), Some(dir)) = (current_section.take(), current_directive.take()) {
                    sections.push(TemplateSection {
                        name: sec,
                        directive: dir,
                    });
                }

                current_section = Some(name);
                current_directive = None;
            }
        }
    }

    // Flush final section
    if let (Some(sec), Some(dir)) = (current_section, current_directive) {
        sections.push(TemplateSection {
            name: sec,
            directive: dir,
        });
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_section() {
        let content = r#"
<!-- section: Overview -->
<!-- directive: Write a brief overview. -->
"#;
        let sections = parse_template(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[0].directive, "Write a brief overview.");
    }

    #[test]
    fn parse_multiple_sections() {
        let content = r#"
# PRD
<!-- section: Overview -->
<!-- directive: Write overview. -->

<!-- section: Requirements -->
<!-- directive: List requirements. -->
"#;
        let sections = parse_template(content);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[1].name, "Requirements");
    }

    #[test]
    fn section_without_directive_is_skipped() {
        let content = r#"
<!-- section: Orphan -->

<!-- section: HasDirective -->
<!-- directive: Write something. -->
"#;
        let sections = parse_template(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "HasDirective");
    }

    #[test]
    fn directive_without_section_is_ignored() {
        let content = r#"
<!-- directive: This has no section. -->

<!-- section: Valid -->
<!-- directive: Write something. -->
"#;
        let sections = parse_template(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Valid");
    }

    #[test]
    fn parses_all_builtin_templates() {
        for tmpl in get_builtin_templates() {
            let sections = parse_template(&tmpl.content);
            assert!(
                !sections.is_empty(),
                "Template '{}' parsed to 0 sections",
                tmpl.name
            );
            for section in &sections {
                assert!(!section.name.is_empty(), "Empty section name in '{}'", tmpl.name);
                assert!(!section.directive.is_empty(), "Empty directive in '{}' section '{}'", tmpl.name, section.name);
            }
        }
    }

    #[test]
    fn prd_has_five_sections() {
        let sections = parse_template(PRD_TEMPLATE);
        assert_eq!(sections.len(), 5);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[4].name, "Success Criteria");
    }

    #[test]
    fn sdd_has_six_sections() {
        let sections = parse_template(SDD_TEMPLATE);
        assert_eq!(sections.len(), 6);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[5].name, "Error Handling");
    }

    #[test]
    fn test_plan_has_five_sections() {
        let sections = parse_template(TEST_PLAN_TEMPLATE);
        assert_eq!(sections.len(), 5);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[4].name, "Acceptance Criteria");
    }

    #[test]
    fn adr_has_four_sections() {
        let sections = parse_template(ADR_TEMPLATE);
        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].name, "Context");
        assert_eq!(sections[3].name, "Alternatives Considered");
    }
}
