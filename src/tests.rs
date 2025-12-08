#[cfg(test)]
mod tests {
    use crate::classifier::{get_classifier, LineType};
    use crate::language::Language;
    use std::fs;

    #[test]
    fn test_python_fixture() {
        let content = fs::read_to_string("src/tests/fixtures/python.py").unwrap();
        let mut classifier = get_classifier(Language::Python);
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(classifier.classify(lines[0]), LineType::Pure); // def hello():
        assert_eq!(classifier.classify(lines[1]), LineType::Pure); // print
        assert_eq!(classifier.classify(lines[2]), LineType::Blank); // empty
        assert_eq!(classifier.classify(lines[3]), LineType::Comment); // # comment
        assert_eq!(classifier.classify(lines[4]), LineType::Docstring); // """
        assert_eq!(classifier.classify(lines[5]), LineType::Docstring); // This is a
        assert_eq!(classifier.classify(lines[6]), LineType::Docstring); // multiline docstring
        assert_eq!(classifier.classify(lines[7]), LineType::Docstring); // """
        assert_eq!(classifier.classify(lines[8]), LineType::Pure); // x = 1
    }

    #[test]
    fn test_html_fixture() {
        let content = fs::read_to_string("src/tests/fixtures/test.html").unwrap();
        let mut classifier = get_classifier(Language::Html);
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(classifier.classify(lines[0]), LineType::Comment); // <!-- start comment -->
        assert_eq!(classifier.classify(lines[1]), LineType::Pure); // <div>
        assert_eq!(classifier.classify(lines[2]), LineType::Pure); // <h1>
        assert_eq!(classifier.classify(lines[3]), LineType::Pure); // <!-- inline --> <p>
        assert_eq!(classifier.classify(lines[4]), LineType::Pure); // </div>
        assert_eq!(classifier.classify(lines[5]), LineType::Comment); // <!--
        assert_eq!(classifier.classify(lines[6]), LineType::Comment); // multiline
        assert_eq!(classifier.classify(lines[7]), LineType::Comment); // comment
        assert_eq!(classifier.classify(lines[8]), LineType::Comment); // -->
    }
}
