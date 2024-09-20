// Jackson Coxson
//
// Code to render the blog from markdown, using GFM syntax
//
// Blog posts are contained inside the forge, at forge/blog (hidden)
// Each folder inside forge/blog is the slug that it should be served
// so that we don't have to hold state in between requests
// Each folder contains the post.md with the post contents, as well
// as a post.toml with information about

pub mod browse;
pub mod page;
pub mod structures;

#[cfg(feature = "ssr")]
mod tests {
    #[allow(unused_imports)]
    use markdown::{CompileOptions, Options, ParseOptions};

    #[test]
    fn t1() {
        let input = include_str!("../../forge/blog/first-post/post.md");
        markdown::CompileOptions::gfm();
        let options = Options {
            parse: ParseOptions::gfm(),
            compile: CompileOptions {
                allow_dangerous_html: true,
                allow_dangerous_protocol: true,
                gfm_footnote_clobber_prefix: Some("".to_string()),
                gfm_tagfilter: true,
                ..CompileOptions::default()
            },
        };
        let output = markdown::to_html_with_options(input, &options).unwrap();
        println!("{output}")
    }
}
