#![feature(convert)]
#![feature(collections)]
#![feature(plugin)]
#![plugin(regex_macros)]

extern crate radix_trie;
extern crate sxd_document;
extern crate collections;
extern crate regex;

use radix_trie::Trie;
use std::io;
use std::io::Read;
use std::fs::File;
use std::io::BufReader;
use regex::Regex;
use collections::BTreeMap;
use sxd_document::parser::Parser;
use sxd_document::dom::{ChildOfRoot, ChildOfElement, Element};

fn errio(text: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, text)
}

fn err<T>(text: &str) -> io::Result<T> {
    Err(errio(text))
}

fn nodeval<'a>(node: &ChildOfElement<'a>) -> io::Result<String> {
    let nodes = match node {
        &ChildOfElement::Element(ref x) => x.children(),
        &ChildOfElement::Text(ref x) => return Ok(x.text().to_string()),
        x => {
            return err(format!("Invalid node provided: {:?}", x).as_str())
        }
    };
    let s = nodes.first().and_then(|x| x.text());
    let s = try!(s.ok_or(errio(format!("Invalid node provided: {:?}", nodes.first()).as_str())));
    Ok(s.text().to_string())
}

static WSP_REGEXP: Regex = regex!(r"(?m)^[ \t\n]+$");
static LSP_REGEXP: Regex = regex!(r"[\n ]\s+");
static ACRO_REGEXP: Regex = regex!(r"(<pos><abr>.*?</pos>)([a-zA-Zа-яА-ЯёЁłŁóÓńŃśŚćĆźŹżŻęĘąĄ]+)");

fn replace_sps(s: &str, first_br: &mut bool) -> String {
    let mut res: String;
    if *first_br {
        res = LSP_REGEXP.replace(s, "");
        if res != s {
            *first_br = false;
        }
    }
    else {
        res = String::from(s);
    }
    res = LSP_REGEXP.replace_all(res.as_str(), "<br/>");
    res = WSP_REGEXP.replace_all(res.as_str(), "");
    res
}

pub struct Xdxf {
    dictionary: Trie<String, String>,
    abbreviations: BTreeMap<String, String>,
}

impl Xdxf {
    fn new() -> Xdxf {
        Xdxf {
            dictionary: Trie::new(),
            abbreviations: BTreeMap::new(),
        }
    }

    fn parse_root<'a>(&mut self, doc: Element<'a>) -> io::Result<()> {
        for child in doc.children() {
            // println!("parse_root: {:?}", child);
            match child {
                ChildOfElement::Text(_) | ChildOfElement::Comment(_) | ChildOfElement::ProcessingInstruction(_) => (),
                ChildOfElement::Element(x) => {
                    match x.name().local_part() {
                        "abbreviations" => try!(self.parse_abbreviations(x)),
                        "ar" => try!(self.add_article(x)),
                        _ => (),
                    }
                },
            }
        };
        Ok(())
    }

    fn format_node<'a>(&self, n: ChildOfElement<'a>, mut title: String, mut first_br: &mut bool) -> (String, String) {
        let mut res = String::new();
        // println!("formatting node (first_br={}): {:?}", first_br, n);
        match n {
            ChildOfElement::Text(x) => {
                res = res + replace_sps(x.text(), &mut first_br).as_str();
            },
            ChildOfElement::Element(x) => {
                match x.name().local_part() {
                    "pos" => {
                        *first_br = false;
                        res = res + "<span class='partofspeech'>";
                        for y in x.children() {
                            let (t, r) = self.format_node(y, title, &mut first_br);
                            title = t;
                            res = res + r.as_str();
                        }
                        res = res + "</span>";
                    },
                    "br" => {
                        if *first_br {
                            res = res + "<br/>";
                        }
                    },
                    "abr" => {
                        *first_br = false;
                        let abbr = x.children().first().unwrap().text().unwrap().text();
                        res = res + "<acronym title='" + self.abbreviations.get(abbr).unwrap().as_str() + "'>" + abbr + "</acronym>";
                    },
                    "k" => title = nodeval(x.children().first().unwrap()).unwrap(),
                    "ar" => {
                        for z in x.children() {
                            let (t, r) = self.format_node(z, title, &mut first_br);
                            title = t;
                            res = res + r.as_str();
                        }
                    },
                    y => {
                        res = res + "<" + y + ">";
                        for z in x.children() {
                            let (t, r) = self.format_node(z, title, &mut first_br);
                            title = t;
                            res = res + r.as_str();
                        }
                        res = res + "</" + y + ">";
                    }
                }
            },
            _ => ()
        };
        (title, res)
    }

    fn add_article<'a>(&mut self, doc: Element<'a>) -> io::Result<()> {
        let (title, content) = self.format_node(ChildOfElement::Element(doc), String::new(), &mut true);
        self.dictionary.insert(title, content);
        Ok(())
    }

    fn parse_abbreviations<'a>(&mut self, doc: Element<'a>) -> io::Result<()> {
        for child in doc.children() {
            match child {
                ChildOfElement::Element(x) => {
                    if x.name().local_part() != "abr_def" {
                        return err("Unexpected abbreviations child")
                    };
                    try!(self.add_abbreviation(x));
                },
                ChildOfElement::Text(_) => (),
                _ => return err("Unexpected abbreviations child"),
            };
        };
        Ok(())
    }

    fn add_abbreviation<'a>(&mut self, abbr: Element<'a>) -> io::Result<()> {
        let mut children = abbr.children();
        if children.len() != 2 {
            return err("Unexpected abbreviation child length");
        }
        let second = children.pop().unwrap();
        let first = children.pop().unwrap();
        let (first, second) = match (first, second) {
            (ChildOfElement::Element(x), ChildOfElement::Element(y))
                if x.name().local_part() == "k" && y.name().local_part() == "v" => (first, second),
            (ChildOfElement::Element(x), ChildOfElement::Element(y))
                if x.name().local_part() == "v" && y.name().local_part() == "k" => (second, first),
            _ => {
                return err("Unexpected abbreviation content");
            }
        };
        self.abbreviations.insert(try!(nodeval(&first)), try!(nodeval(&second)));
        Ok(())
    }

    pub fn load_file(path: &str) -> io::Result<Xdxf>{
        let mut xml = String::new();
        {
            let mut file = BufReader::new(try!(File::open(path)));
            try!(file.read_to_string(&mut xml));
        }
        Xdxf::load_str(xml.as_str())
    }

    pub fn load_str(data: &str) -> io::Result<Xdxf>{
        let mut dict = Xdxf::new();
        try!(dict.feed_str(data));
        Ok(dict)
    }

    pub fn feed_file(&mut self, path: &str) -> io::Result<()> {
        let mut xml = String::new();
        {
            let mut file = BufReader::new(try!(File::open(path)));
            try!(file.read_to_string(&mut xml));
        }
        self.feed_str(xml.as_str())
    }

    pub fn feed_str(&mut self, data: &str) -> io::Result<()> {
        let data = ACRO_REGEXP.replace_all(data, "$2$1");
        let package = try!(Parser::new().parse(data.trim()).or(err("Malformed XML")));
        let doc = package.as_document().root();
        for child in doc.children() {
            match child {
                ChildOfRoot::Element(x) if x.name().local_part() == "xdxf" => try!(self.parse_root(x)),
                _ => (),
            }
        };
        Ok(())
    }

    pub fn lookup(&self, prefix: &str) -> Vec<(String, String)> {
        if prefix.len() < 3 {
            return Vec::new()
        };
        let n = match self.dictionary.get_descendant(&prefix.to_string()) {
            None => return Vec::new(),
            Some(x) => x,
        };
        let mut res = Vec::new();
        for (k, v) in n.iter() {
            res.push((k.to_string(), v.to_string()));
        };
        res
    }
}

#[cfg(test)]
mod test {
    use super::Xdxf;

    static DICT_EXAMPLE: &'static str = "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>
<xdxf lang_from=\"POL\" lang_to=\"RUS\" format=\"visual\">
    <full_name>Polish-Russian Dictionary</full_name>
    <description>Created by torkve, on the base of AndrewM's dictionary for SDictionary project</description>
    <abbreviations>
        <abr_def><k>f</k><v>rodzaj żeński</v></abr_def>
        <abr_def><k>m</k><v>rodzaj męski</v></abr_def>
        <abr_def><k>n</k><v>rodzaj nijaki</v></abr_def>
        <abr_def><k>rzecz.</k><v>rzeczownik</v></abr_def>
        <abr_def><k>przym.</k><v>przymiotnik</v></abr_def>
    </abbreviations>
    <ar><k>żółwi</k>
        żó<pos><abr>rzecz.</abr></pos>łw <i><abr>m</abr></i>
        черепаха <i><abr>f</abr></i>
        żó<pos><abr>przym.</abr></pos>łwi
        черепашечный
         <small><i>Biologiczny Przenośny</i></small> черепаший</ar>
    <ar><k>żółwica</k>
        żó<pos><abr>rzecz.</abr></pos>łwica <i><abr>f</abr></i>
        черепаха <i><abr>f</abr></i></ar>
</xdxf> 
";

    #[test]
    pub fn test_parser() {
        let dict = Xdxf::load_str(DICT_EXAMPLE).unwrap();
        assert_eq!(dict.abbreviations.get("m").unwrap(), "rodzaj męski");

        let nodes = dict.lookup("żół");
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].0, "żółwi");
        assert_eq!(nodes[0].1, "żó<span class='partofspeech'><acronym title='rzeczownik'>rzecz.</acronym></span>łw <i><acronym title='rodzaj męski'>m</acronym></i><br/>черепаха <i><acronym title='rodzaj żeński'>f</acronym></i><br/>żó<span class='partofspeech'><acronym title='przymiotnik'>przym.</acronym></span>łwi<br/>черепашечный<br/><small><i>Biologiczny Przenośny</i></small> черепаший");
    }
}
