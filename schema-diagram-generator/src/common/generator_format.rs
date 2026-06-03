use std::str::FromStr;
use crate::common::diagram_generator::DiagramGenerator;
use crate::common::generate_options::DiagramGenerateOptions;
use crate::mermaid::mermaid_generator::MermaidERDiagramGenerator;
use crate::plantuml::plantuml_generator::PlantUMLERDiagramGenerator;

pub enum DiagramFormat {
    Mermaid,
    PlantUml,
}

impl FromStr for DiagramFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mermaid" => Ok(DiagramFormat::Mermaid),
            "plantuml" => Ok(DiagramFormat::PlantUml),
            _ => Err(format!("Unknown diagram format: {}", s)),
        }
    }
}

impl DiagramFormat {
    pub fn generate(&self, options: DiagramGenerateOptions) -> String {
        match self {
            DiagramFormat::Mermaid => {
                let generator = MermaidERDiagramGenerator::new(options.database_model);
                generator.generate()
            }
            DiagramFormat::PlantUml => {
                let generator = PlantUMLERDiagramGenerator::new(options.database_model);
                generator.generate()
            }
        }
    }

    pub fn file_extension(&self) -> &'static str {
        "md"
    }

    pub fn format_name(&self) -> &'static str {
        match self {
            DiagramFormat::Mermaid => "mermaid",
            DiagramFormat::PlantUml => "plantuml",
        }
    }
}
