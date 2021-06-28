use clap::{App, AppSettings, Arg};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::{self, prelude::*};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum CommentType {
    SectionBegin,
    SectionEnd,
    SourceInfo,
    HashInfo,
}

#[derive(Clone)]
pub struct Specialcomment {
    line: u32,
    content: String,
    section: String,
    ctype: CommentType,
    argument: Option<String>,
}

pub struct ContentLine {
    linenumber: u32,
    content: String,
}

impl Specialcomment {

    fn new(line: &str, commentsymbol: &str, linenumber: u32) -> Option<Specialcomment> {
        let mut iscomment = String::from("^ *");
        iscomment.push_str(&commentsymbol);
        iscomment.push_str(" *... *(.*)");

        let commentregex = Regex::new(&iscomment).unwrap();

        let keywords = commentregex.captures(&line);
        match &keywords {
            Some(captures) => {
                let keywords = captures
                    .get(1)
                    .unwrap()
                    .as_str()
                    .split(" ")
                    .collect::<Vec<&str>>();

                // needs at least a section and a keyword
                if keywords.len() < 2 {
                    return Option::None;
                }

                let sectionname = keywords[0];
                let keyword = keywords[1];
                let cargument: Option<String>;

                if keywords.len() > 2 {
                    cargument = Option::Some(String::from(keywords[2]));
                } else {
                    cargument = Option::None;
                }

                let tmptype: CommentType;
                match keyword {
                    "begin" => {
                        tmptype = CommentType::SectionBegin;
                    }
                    "end" => {
                        tmptype = CommentType::SectionEnd;
                    }
                    "hash" => {
                        tmptype = CommentType::HashInfo;
                        match cargument {
                            Some(_) => {}
                            None => {
                                println!("missing hash value on line {}", linenumber);
                                return Option::None;
                            }
                        }
                    }
                    "source" => {
                        tmptype = CommentType::SourceInfo;
                        match cargument {
                            Some(_) => {}
                            None => {
                                println!("missing source file on line {}", linenumber);
                                return Option::None;
                            }
                        }
                    }

                    &_ => {
                        println!("warning: incomplete imosid comment on {}", linenumber);
                        return Option::None;
                    }
                }

                Option::Some(Specialcomment {
                    line: linenumber,
                    content: String::from(line),
                    section: String::from(sectionname),
                    ctype: tmptype,
                    argument: cargument,
                })
            }
            None => {
                return Option::None;
            }
        }
    }
}

pub struct Section {
    startline: u32,
    name: Option<String>,
    source: Option<String>,
    endline: u32,
    hash: String,
    content: String,
    broken: bool,
}

impl Section {
    fn new(start: u32, end: u32, name: Option<String>, source: Option<String>) -> Section {
        Section {
            name: name,
            startline: start,
            endline: end,
            source: source,
            hash: String::from(""), //todo
            broken: false,
            content: String::new(),
        }
    }

    fn push_str(&mut self, line: &str) {
        self.content.push_str(line);
        self.content.push('\n');
    }

    fn output(&self, commentsign: &str) -> String {
        let mut outstr = String::new();
        match &self.name {
            Some(name) => {
                outstr.push_str(&format!("{}... {} begin\n", commentsign, name));
                outstr.push_str(&format!("{}... {} begin\n", commentsign, self.hash));
                match &self.source {
                    Some(source) => {
                        outstr.push_str(&format!("{}... {} begin\n", commentsign, source));
                    }
                    None => {}
                } //todo: section target
            }
            // anonymous section
            None => {
                outstr = self.content.clone();
                return outstr;
            }
        }
        return outstr;
    }
}

pub struct Specialfile {
    content: String,
    specialcomments: Vec<Specialcomment>,
    sections: Vec<Section>,
    file: File,
}

impl Specialfile {
    fn new(filename: &str) -> Specialfile {
        let sourcefile = OpenOptions::new()
            .read(true)
            .write(true)
            .open(filename)
            .unwrap();

        let mut commentvector = Vec::new();
        let mut counter = 0;

        let mut sectionvector: Vec<Section> = Vec::new();
        let mut contentvector: Vec<ContentLine> = Vec::new();

        let mut sectionmap: HashMap<String, Vec<Specialcomment>> = HashMap::new();

        let filelines = io::BufReader::new(&sourcefile).lines();

        for i in filelines {
            counter += 1;
            let line = i.unwrap();
            let newcomment = Specialcomment::new(&line, "#", counter);
            match newcomment {
                Some(comment) => {
                    commentvector.push(comment.clone());
                    if sectionmap.contains_key(&comment.section) {
                        sectionmap.get_mut(&comment.section).unwrap().push(comment);
                    } else {
                        let mut sectionvector = Vec::new();
                        sectionvector.push(comment.clone());
                        sectionmap.insert(comment.section, sectionvector);
                    }
                }
                None => contentvector.push(ContentLine {
                    linenumber: counter,
                    content: line,
                }),
            }
        }

        for (sectionname, svector) in sectionmap.iter() {
            let mut checkmap = HashMap::new();
            for i in svector.iter() {
                if checkmap.contains_key(&i.ctype) {
                    break;
                } else {
                    checkmap.insert(&i.ctype, i);
                }
            }
            if !(checkmap.contains_key(&CommentType::SectionBegin)
                && checkmap.contains_key(&CommentType::SectionEnd))
            {
                println!("warning: invalid section {}", sectionname);
                continue;
            }

            let newsection = Section::new(
                checkmap.get(&CommentType::SectionBegin).unwrap().line,
                checkmap.get(&CommentType::SectionEnd).unwrap().line,
                Option::Some(String::from(sectionname)),
                Option::None, //todo
            );

            sectionvector.push(newsection);
        }

        sectionvector.sort_by(|a, b| a.startline.cmp(&b.startline));

        let mut currentline = 1;
        let mut tmpstart;
        let mut tmpend;
        let mut anonvector: Vec<Section> = Vec::new();
        for i in &sectionvector {
            if i.startline - currentline >= 1 {
                tmpstart = currentline;
                tmpend = i.startline - 1;
                let newsection = Section::new(tmpstart, tmpend, Option::None, Option::None);
                anonvector.push(newsection);
            }
            currentline = i.endline + 1;
        }

        for i in &mut sectionvector {
            // TODO: speed this up, binary search or something
            for c in &contentvector {
                if c.linenumber >= i.endline {
                    break;
                } else if c.linenumber < i.startline {
                    continue;
                }
                i.push_str(&c.content);
            }
        }

        println!("{}", contentvector.len());
        sectionvector.extend(anonvector);
        sectionvector.sort_by(|a, b| a.startline.cmp(&b.startline));

        let retfile = Specialfile {
            content: String::new(),
            specialcomments: commentvector,
            sections: sectionvector,
            file: sourcefile,
        };
        return retfile;
    }
}

fn main() {
    let inputarg = Arg::new("input")
        .multiple_occurrences(true)
        .short('i')
        .long("input")
        .takes_value(true)
        .required(false)
        .about("add file to source list");

    let matches = App::new("imosid")
        .version("0.1")
        .author("paperbenni <paperbenni@gmail.com>")
        .about("instant manager of sections in dotfiles")
        .arg(Arg::new("syntax").required(false).about("manually set the comment syntax"))
        .subcommand(
            App::new("update")
                .about("apply source sections to target")
                .arg(
                    inputarg
                ).arg(
                    Arg::new("target")
                        .index(1)
                        .required(true)
                        .about("file to apply updates to")
                ).arg(
                    Arg::new("section").long("section")
                        .about("only update section <section>. all sections are included if unspecified")
                        .multiple_occurrences(true).takes_value(true).required(false)
                ).setting(AppSettings::ColoredHelp),
        ).subcommand(
            App::new("compile")
                .about("add hashes to sections in source file")
                .setting(AppSettings::ColoredHelp)
                .arg(
                    Arg::new("file")
                        .index(1)
                        .required(true)
                        .about("file to process")
                )
        ).subcommand(
            App::new("query")
                .about("print section from file")
                .arg(
                    Arg::new("file")
                        .index(1)
                        .about("file to search through")
                        .required(true)
                ).arg(Arg::new("section").index(2).required(false).short('s')),
        )
        .setting(AppSettings::ColoredHelp)
        .get_matches();

    if matches.is_present("compile") {
        if let Some(ref matches) = matches.subcommand_matches("compile") {
            let filename = matches.value_of("file").unwrap();
            println!("{}", filename);

            if Path::new(filename).is_file() {
                let mut linevec = Vec::new();
                let mut contentstring = String::new();
                let queryfile = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(filename)
                    .unwrap();
                let lines = io::BufReader::new(queryfile).lines();
                for i in lines {
                    let line = i.unwrap();
                    linevec.push(line.clone());
                    contentstring.push_str(&line);
                    contentstring.push('\n');
                }
                let mut hasher = Sha256::new();
                hasher.update(contentstring);
                let hasher = hasher.finalize();
                println!("{:X}", hasher);
            }
        }
    }

    let testfile = Specialfile::new("tester.txt");
    println!("argument {}", testfile.sections[1].content);
    println!("len {}", testfile.sections.len());
}
