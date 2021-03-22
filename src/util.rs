use ego_tree::NodeRef;
use scraper::Node;

pub struct NodeIterFormatter<'a, I: Iterator<Item = NodeRef<'a, Node>> + 'a + Sized>(
    std::cell::RefCell<I>,
);

impl<'a, I: Iterator<Item = NodeRef<'a, Node>> + 'a + Sized> NodeIterFormatter<'a, I> {
    pub fn new(itr: I) -> Self {
        Self(std::cell::RefCell::new(itr))
    }
}

impl<'a, I: Iterator<Item = NodeRef<'a, Node>> + 'a + Sized> std::fmt::Display
    for NodeIterFormatter<'a, I>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut itr = self.0.borrow_mut();
        for node in &mut *itr {
            match node.value() {
                Node::Text(text) => write!(f, "{}", text.trim())?,
                Node::Element(elem) if elem.name() == "br" => writeln!(f)?,
                _ => {}
            }
        }

        Ok(())
    }
}
