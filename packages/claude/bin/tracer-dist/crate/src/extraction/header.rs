use tree_sitter::Node;

pub struct Builder<'a> {
    source: &'a [u8],
    parts: Vec<(usize, usize, Option<&'static str>)>,
}

impl<'a> Builder<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            parts: Vec::new(),
        }
    }

    pub fn slice(&mut self, start_byte: usize, end_byte: usize) -> &mut Self {
        self.parts.push((start_byte, end_byte, None));
        self
    }

    pub fn node(&mut self, node: Node) -> &mut Self {
        self.slice(node.start_byte(), node.end_byte())
    }

    pub fn block(&mut self, node: Node) -> &mut Self {
        self.parts
            .push((node.start_byte(), node.end_byte(), Some("{ … }")));
        self
    }

    pub fn expression(&mut self, node: Node) -> &mut Self {
        self.parts
            .push((node.start_byte(), node.end_byte(), Some("…")));
        self
    }

    pub fn finish(&self) -> String {
        let mut out = String::new();
        let mut end = None;
        for (start, stop, replacement) in &self.parts {
            let text = replacement.map(str::to_string).unwrap_or_else(|| {
                String::from_utf8_lossy(&self.source[*start..*stop]).to_string()
            });
            if let Some(previous) = end.filter(|previous| *previous < *start) {
                let gap = &self.source[previous..*start];
                if gap.contains(&b'\n')
                    && gap.iter().all(|byte| byte.is_ascii_whitespace())
                    && replacement.is_none()
                {
                    out.push('\n');
                } else if !out.ends_with(char::is_whitespace) && !text.starts_with(';') {
                    out.push(' ');
                }
            }
            if replacement.is_some() && !out.is_empty() {
                out.truncate(out.trim_end().len());
                // `Array<{ … }>` and `f({ … })` hug their bracket.
                if !out.ends_with(['<', '(', '[']) {
                    out.push(' ');
                }
            }
            out.push_str(&text);
            end = Some(*stop);
        }
        let source_indentation = self
            .parts
            .first()
            .map(|(start, _, _)| {
                let line_start = self.source[..*start]
                    .iter()
                    .rposition(|byte| *byte == b'\n')
                    .map_or(0, |index| index + 1);
                self.source[line_start..*start]
                    .iter()
                    .take_while(|byte| matches!(**byte, b' ' | b'\t'))
                    .count()
            })
            .unwrap_or(0);
        let indentation = source_indentation.max(
            out.lines()
                .next()
                .map(|line| line.len() - line.trim_start().len())
                .unwrap_or(0),
        );
        out.lines()
            .enumerate()
            .map(|(index, line)| {
                let leading = line.len() - line.trim_start().len();
                let trim = if index == 0 && leading < indentation {
                    0
                } else {
                    indentation.min(leading)
                };
                line.get(trim..).unwrap_or(line).trim_end()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
