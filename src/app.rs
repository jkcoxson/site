use crate::{
    blog,
    error_template::{AppError, ErrorTemplate},
    forge_component::ForgeComponent,
};
use chrono::Datelike;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use rand::{rngs::ThreadRng, Rng};

#[component]
/// The root of the application
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/cdn/site/pkg/jkcoxson.css" />
        <Script
            id="highlighter-js"
            src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"
        />
        <Script id="main-js" src="/cdn/site/js/main.js" />

        <Eruda />

        // sets the document title
        <Title text="Jackson Coxson" />

        // content for this welcome page
        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! {
                <NavBar />
                <ErrorTemplate outside_errors />
                <Footer />
            }
                .into_view()
        }>
            <main class="relative z-0 dark:bg-stone-900">
                <Routes>
                    <Route path="" view=HomePage />
                    <Route path="/forge/*any" view=ForgeComponent />
                    <Route path="/blog" view=blog::browse::BrowseView />
                    <Route path="/blog/:id" view=blog::page::PageView />
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    // Creates a reactive value to update the button
    // let (count, set_count) = create_signal(0);
    // let on_click = move |_| set_count.update(|count| *count += 1);

    view! {
        <NavBar />
        <HeroSection />
        <AboutSection />
        <hr />
        <Toolbox />
        <Projects />
        <Contact />
        <BlogShowcase />
        <Footer />
    }
}

/// Nav bar
#[component]
pub fn NavBar() -> impl IntoView {
    view! {
        <nav class="flex bg-white  dark:bg-stone-900">
            <div class="container mx-auto flex items-start justify-between">
                <a class="flex items-start justify-start m-4" href="/">
                    <span class="me-2 flex items-center justify-center rounded-full bg-transparent">
                        <img
                            src="/cdn/site/img/transparent.png"
                            alt="logo"
                            width="53"
                            height="53"
                        />
                    </span>
                </a>
                <div class="m-2 grid justify-items-end p-2">
                    <button
                        class="p-2 lg:hidden"
                        aria-controls="navcol-2"
                        aria-expanded="false"
                        onclick="toggleMenu()"
                    >
                        <span class="sr-only">Toggle navigation</span>
                        <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24">
                            <path
                                class="stroke-cyan-400 stroke-2 dark:stroke-green-800"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                stroke-width="2"
                                d="M4 6h16M4 12h16m-7 6h7"
                            />
                        </svg>
                    </button>
                    <div class="hidden lg:flex lg:items-center" id="navcol-2">
                        <ul class="ml-auto space-y-4 lg:space-y-0 lg:flex lg:space-x-4">
                            <li>
                                <a
                                    class="font-alatsi text-gray-800 dark:text-gray-200"
                                    href="/blog"
                                >
                                    Blog
                                </a>
                            </li>
                            <li>
                                <a
                                    class="font-alatsi text-gray-800 dark:text-gray-200"
                                    href="/forge"
                                >
                                    Forge
                                </a>
                            </li>
                            <li>
                                <div class="px-2">
                                    <a href="https://github.com/jkcoxson" target="_blank">
                                        <img
                                            class="bg-light h-10 w-10 rounded-full border dark:bg-white"
                                            src="/cdn/site/img/github-mark.png"
                                            alt="GitHub"
                                        />
                                    </a>
                                </div>
                            </li>
                            <li>
                                <a class="m-2 rounded-md bg-blue-500 px-4 py-2 text-white" href="#">
                                    Say Hi
                                </a>
                            </li>
                        </ul>
                    </div>
                </div>
            </div>
        </nav>
    }
}

#[component]
/// Hero section of the app
fn HeroSection() -> impl IntoView {
    view! {
        <section class="content-center">
            <div class="mx-auto">
                <div class="flex h-screen">
                    <div class="mx-auto flex flex-col content-center justify-center text-center">
                        <div class="">
                            <h2
                                class="text-uppercase mb-3 text-3xl font-bold dark:text-stone-400"
                                style="font-family: 'Roboto', sans-serif;"
                            >
                                Innovator, Engineer & Programmer
                            </h2>
                            <p class="mb-4 dark:text-stone-400">
                                I design powerful systems and push the limits of technology.
                            </p>
                            <button
                                class="mr-2 rounded bg-blue-500 px-4 py-2 text-lg font-semibold text-white dark:text-white"
                                type="button"
                            >
                                Contact
                            </button>
                            <button
                                class="rounded border border-blue-500 px-4 py-2 text-lg font-semibold text-blue-500"
                                type="button"
                            >
                                Portfolio
                            </button>
                        </div>
                    </div>
                </div>
            </div>
            <TraceSvg />
        </section>
    }
}

#[component]
fn TraceSvg() -> impl IntoView {
    let mut rng = rand::thread_rng();
    view! {
        <svg
            class="absolute left-0 right-0 top-0 -z-50"
            xmlns="http://www.w3.org/2000/svg"
            width="80%"
            height="220vh"
        >
            <path
                d=generate_trace(&mut rng)
                class="stroke-dasharray-[0,100%] animate-draw fill-none stroke-cyan-200 stroke-2 dark:stroke-green-800"
            ></path>
            <path
                d=generate_trace(&mut rng)
                class="stroke-dasharray-[0,100%] animate-draw fill-none stroke-cyan-200 stroke-2 dark:stroke-green-800"
            ></path>
            <path
                d=generate_trace(&mut rng)
                class="stroke-dasharray-[0,100%] animate-draw fill-none stroke-cyan-200 stroke-2 dark:stroke-green-800"
            ></path>
            <path
                d=generate_trace(&mut rng)
                class="stroke-dasharray-[0,100%] animate-draw fill-none stroke-cyan-200 stroke-2 dark:stroke-green-800"
            ></path>
            <path
                d=generate_trace(&mut rng)
                class="stroke-dasharray-[0,100%] animate-draw fill-none stroke-cyan-200 stroke-2 dark:stroke-green-800"
            ></path>
            <path
                d=generate_trace(&mut rng)
                class="stroke-dasharray-[0,100%] animate-draw fill-none stroke-cyan-200 stroke-2 dark:stroke-green-800"
            ></path>
            <path
                d=generate_trace(&mut rng)
                class="stroke-dasharray-[0,100%] animate-draw fill-none stroke-cyan-200 stroke-2 dark:stroke-green-800"
            ></path>
            <path
                d=generate_trace(&mut rng)
                class="stroke-dasharray-[0,100%] animate-draw fill-none stroke-cyan-200 stroke-2 dark:stroke-green-800"
            ></path>
            <path
                d=generate_trace(&mut rng)
                class="stroke-dasharray-[0,100%] animate-draw fill-none stroke-cyan-200 stroke-2 dark:stroke-green-800"
            ></path>
        </svg>
    }
}

fn generate_trace(rng: &mut ThreadRng) -> String {
    let mut x = 400;
    let mut y = 400;
    let mut trace = "M 400 400".to_string();

    let movements: u16 = rng.gen_range(120..=200);
    let start = rng.gen_range(0..=1);
    for m in start..movements {
        if m % 2 == 0 {
            x = rng.gen_range((x / 50) - 6..(x / 50) + 6) * 50;
        } else {
            y = rng.gen_range((y / 50) - 2..(y / 50) + 2) * 50;
        }
        trace = format!(" {trace} L {x}, {y}");
    }

    trace
}

#[component]
/// About section
fn AboutSection() -> impl IntoView {
    view! {
        <div class="m-2 mb-8 flex flex-wrap items-center justify-center">
            <div class="flex w-full flex-col md:w-1/2 lg:w-1/3">
                <h1 class="text-center text-3xl font-bold">Hi, I am Jackson!</h1>
                <p class="">
                    I am an engineer, dreamer and <strong>
                        <span class="underline">innovator</span>
                    </strong>
                    . Pushing the limits of technology has been my passion since I was young, and I am also a strong believer in the power of
                    <strong>
                        <span class="underline">open-source</span>
                    </strong>
                    and have contributed to various projects in the community. I am always looking for new challenges and opportunities to learn and
                    <strong>
                        <span class="underline">grow</span>
                    </strong>.
                </p>
            </div>
            <div class="flex w-full flex-wrap justify-center md:w-1/2 lg:w-1/3">
                <img
                    class="w-1/2 rounded-full object-cover"
                    alt="profile"
                    src="/cdn/site/img/profile.jpg"
                />
            </div>
        </div>
    }
}

#[component]
/// Toolbox
fn Toolbox() -> impl IntoView {
    view! {
        <div class="m-5 mx-0 bg-gray-200 p-5 dark:bg-stone-800">
            <h1 class="text-center font-bold">Toolbox</h1>
            <div class="flex flex-col">
                <div class="flex flex-wrap items-center justify-center">
                    <div class="flex w-full justify-center"></div>
                    <Tool name="Rust".to_string() img_src="/cdn/site/img/rust.png".to_string() />
                    <Tool
                        name="Javascript".to_string()
                        img_src="https://upload.wikimedia.org/wikipedia/commons/thumb/6/6a/JavaScript-logo.png/600px-JavaScript-logo.png?20120221235433"
                            .to_string()
                    />
                    <Tool
                        name="Python".to_string()
                        img_src="https://s3.dualstack.us-east-2.amazonaws.com/pythondotorg-assets/media/community/logos/python-logo-only.png"
                            .to_string()
                    />
                    <Tool
                        name="Go".to_string()
                        img_src="https://upload.wikimedia.org/wikipedia/commons/thumb/0/05/Go_Logo_Blue.svg/512px-Go_Logo_Blue.svg.png?20191207190041"
                            .to_string()
                    />
                    <Tool
                        name="Svelte".to_string()
                        img_src="https://github.com/sveltejs/branding/blob/master/svelte-logo.png?raw=true"
                            .to_string()
                    />
                    <Tool
                        name="MySQL".to_string()
                        img_src="https://upload.wikimedia.org/wikipedia/commons/thumb/0/0a/MySQL_textlogo.svg/800px-MySQL_textlogo.svg.png?20210508081050"
                            .to_string()
                    />
                    <Tool
                        name="Git".to_string()
                        img_src="https://git-scm.com/images/logos/downloads/Git-Icon-1788C.png"
                            .to_string()
                    />
                    <div
                        class="mx-4 flex flex-wrap items-center justify-center text-center"
                        data-bss-hover-animate="rubberBand"
                    >
                        <h1 class="m-2 mb-2 flex h-20 w-20 items-center justify-center rounded-lg bg-gray-800 text-4xl text-white">
                            >_
                        </h1>
                        <h1 class="text-center">Linux</h1>
                    </div>
                </div>
            </div>
            <small class="pl-0 text-xs dark:text-gray-200">
                Individual logos are trademarked property. I am not affiliated with any
                of these organizations, nor am I implying any sort of sponsorship. I
                doubt they know I exist. These are merely tools that I am proficient in.
                Rust logo:
                <a
                    href="https://foundation.rust-lang.org/policies/logo-policy-and-media-guide/"
                    target="_blank"
                    class="text-blue-600 underline"
                >
                    Rust Foundation
                </a>- Python logo:
                <a
                    href="https://www.python.org/psf/trademarks/"
                    target="_blank"
                    class="text-blue-600 underline"
                >
                    PSF
                </a>-
                <a
                    href="https://github.com/sveltejs/branding"
                    target="_blank"
                    class="text-blue-600 underline"
                >
                    Svelte
                </a>- Git logo:
                <a
                    href="https://git-scm.com/downloads/logos"
                    target="_blank"
                    class="text-blue-600 underline"
                >
                    Jason Long
                </a>- Go:
                <a href="https://go.dev/brand" target="_blank" class="text-blue-600 underline">
                    Google
                </a>-
                <a
                    href="https://commons.wikimedia.org/wiki/File:MySQL_textlogo.svg"
                    target="_blank"
                    class="text-blue-600 underline"
                >
                    MySQL
                </a>
            </small>
        </div>
    }
}

#[component]
fn Tool(name: String, img_src: String) -> impl IntoView {
    view! {
        <div class="m-6 mx-4 flex flex-wrap items-center justify-center text-center">
            <h1 class="flex items-center text-center text-4xl">
                <img src=img_src width="80" class="mr-2" alt="" />
                {name}
            </h1>
        </div>
    }
}

#[component]
/// Projects section of the home page
fn Projects() -> impl IntoView {
    view! {
        <div class="w-screen">
            <div class="mb-5 text-center">
                <h2 class="font-bold">Projects</h2>
                <p class="mx-auto w-full lg:w-1/2">
                    Here is a small taste of the work {"I've"} done
                </p>
            </div>
            <div class="m-6 grid grid-cols-1 justify-center gap-4 md:m-24 md:grid-cols-2 xl:grid-cols-3">
                <Project
                    title="JitStreamer".to_string()
                    description="JitStreamer was a service that enabled users to exploit a loophole in the iOS developer stack."
                        .to_string()
                    image="/cdn/site/img/jitstreamer.jpg".to_string()
                    link="https://github.com/jkcoxson/JitStreamer".to_string()
                />
                <Project
                    title="SideStore".to_string()
                    description="SideStore uses a custom IP stack to install apps on iOS without the App Store."
                        .to_string()
                    image="/cdn/site/img/sidestore.jpeg".to_string()
                    link="https://github.com/SideStore".to_string()
                />
                <Project
                    title="MoabDB".to_string()
                    description="MoabDB is a finance database and API for traders and researches to view histories and make predictions."
                        .to_string()
                    image="/cdn/site/img/moabdb.png".to_string()
                    link="https://moabdb.com".to_string()
                />
            </div>
        </div>
    }
}

#[component]
fn Project(title: String, description: String, image: String, link: String) -> impl IntoView {
    view! {
        <div class="flex content-center items-center">
            <a href=link target="_blank" class="text-gray-800">
                <div class="transform rounded-lg bg-gray-200 transition-transform hover:scale-105 dark:bg-gray-800">
                    <div class="p-4">
                        <img
                            src=image
                            alt=title.clone()
                            class="mb-3 max-h-40 rounded-lg object-cover"
                        />
                        <h4 class="text-lg font-semibold">{title}</h4>
                        <p class="text-gray-600 dark:text-gray-200">{description}</p>
                    </div>
                </div>
            </a>
        </div>
    }
}

#[component]
/// Contact me tile
fn Contact() -> impl IntoView {
    view! {
        <section class="relative m-6 py-16 md:m-44 lg:py-24">
            <div class="relative">
                <div class="mb-10 text-center">
                    <h2 class="text-3xl font-bold">Get in Contact</h2>
                </div>
                <div class="flex flex-wrap justify-center">
                    <div class="mb-8 w-full md:w-1/2 lg:w-1/3">
                        <div class="flex h-full flex-col justify-center">
                            <div class="flex items-center p-4">
                                <div class="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-full bg-blue-500 text-white">
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        width="1em"
                                        height="1em"
                                        fill="currentColor"
                                        viewBox="0 0 16 16"
                                        class="bi bi-hash"
                                    >
                                        <path d="M8.39 12.648a1.32 1.32 0 0 0-.015.18c0 .305.21.508.5.508.266 0 .492-.172.555-.477l.554-2.703h1.204c.421 0 .617-.234.617-.547 0-.312-.188-.53-.617-.53h-.985l.516-2.524h1.265c.43 0 .618-.227.618-.547 0-.313-.188-.524-.618-.524h-1.046l.476-2.304a1.06 1.06 0 0 0 .016-.164.51.51 0 0 0-.516-.516.54.54 0 0 0-.539.43l-.523 2.554H7.617l.477-2.304c.008-.04.015-.118.015-.164a.512.512 0 0 0-.523-.516.539.539 0 0 0-.531.43L6.53 5.484H5.414c-.43 0-.617.22-.617.532 0 .312.187.539.617.539h.906l-.515 2.523H4.609c-.421 0-.609.219-.609.531 0 .313.188.547.61.547h.976l-.516 2.492c-.008.04-.015.125-.015.18 0 .305.21.508.5.508.265 0 .492-.172.554-.477l.555-2.703h2.242l-.515 2.492zm-1-6.109h2.266l-.515 2.563H6.859l.532-2.563z"></path>
                                    </svg>
                                </div>
                                <div class="ml-4">
                                    <h6 class="text-lg font-semibold dark:text-white">Discord</h6>
                                    <p class="text-gray-600 dark:text-gray-200">@jkcoxson</p>
                                </div>
                            </div>
                            <div class="flex items-center p-4">
                                <div class="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-full bg-blue-500 text-white">
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        width="1em"
                                        height="1em"
                                        fill="currentColor"
                                        viewBox="0 0 16 16"
                                        class="bi bi-envelope"
                                    >
                                        <path d="M0 4a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2zm2-1a1 1 0 0 0-1 1v.217l7 4.2 7-4.2V4a1 1 0 0 0-1-1zm13 2.383-4.708 2.825L15 11.105zm-.034 6.876-5.64-3.471L8 9.583l-1.326-.795-5.64 3.47A1 1 0 0 0 2 13h12a1 1 0 0 0 .966-.741M1 11.105l4.708-2.897L1 5.383z"></path>
                                    </svg>
                                </div>
                                <div class="ml-4">
                                    <h6 class="text-lg font-semibold dark:text-white">Email</h6>
                                    <p class="text-gray-600 dark:text-gray-200">Loading...</p>
                                </div>
                            </div>
                        </div>
                    </div>
                    <div class="w-full md:w-1/2 lg:w-2/3">
                        <div>
                            <form
                                class="rounded-lg bg-white p-4 shadow-2xl dark:bg-stone-800"
                                method="post"
                            >
                                <div class="mb-4">
                                    <input
                                        class="form-input w-full rounded border p-3 dark:border-black dark:bg-stone-700"
                                        type="text"
                                        id="name-1"
                                        name="name"
                                        placeholder="Name"
                                    />
                                </div>
                                <div class="mb-4">
                                    <input
                                        class="form-input w-full rounded border p-3 dark:border-black dark:bg-stone-700"
                                        type="email"
                                        id="email-1"
                                        name="email"
                                        placeholder="Email"
                                    />
                                </div>
                                <div class="mb-4">
                                    <textarea
                                        class="form-input w-full rounded border p-3 dark:border-black dark:bg-stone-700"
                                        id="message-1"
                                        name="message"
                                        rows="6"
                                        placeholder="Message"
                                    ></textarea>
                                </div>
                                <div>
                                    <button
                                        class="w-full rounded bg-blue-500 py-3 text-white transition hover:bg-blue-600"
                                        type="submit"
                                    >
                                        Send
                                    </button>
                                </div>
                            </form>
                        </div>
                    </div>
                </div>
            </div>
        </section>
    }
}

#[component]
/// Shows a few of the most recent blog posts
fn BlogShowcase() -> impl IntoView {
    let once = create_resource(
        || (),
        |_| async move { blog::browse::get_posts(None, Some(3)).await },
    );
    view! {
        <div class="bg-cyan-200 py-16 lg:py-24 dark:bg-cyan-950">
            <div class="mb-10 text-center">
                <div class="font-bold text-gray-600 dark:text-gray-200">
                    <h2 class="mb-2">Blog</h2>
                    <p class="mx-auto md:w-1/2">
                        Read about my findings and work! I am passionate about technology and
                        not afraid to share my opinion.
                    </p>
                </div>
            </div>
            <div class="m-6 grid grid-cols-1 gap-6 md:m-24 md:grid-cols-2 lg:grid-cols-3">
                <Suspense fallback=move || {
                    view! { <h2>"Loading..."</h2> }
                }>
                    {move || match once.get() {
                        Some(posts) => {
                            match posts {
                                Ok(posts) => {
                                    view! {
                                        {posts
                                            .into_iter()
                                            .map(|p| {
                                                view! { <BlogShowcaseItem preview=p /> }
                                            })
                                            .collect::<Vec<_>>()
                                            .into_view()}
                                    }
                                        .into_view()
                                }
                                Err(e) => {
                                    println!("Error fetching posts: {e:?}");
                                    let mut outside_errors = Errors::default();
                                    outside_errors
                                        .insert_with_default_key(AppError::InternalServerError);
                                    view! { <ErrorTemplate outside_errors /> }.into_view()
                                }
                            }
                        }
                        None => view! { <h2>"Loading..."</h2> }.into_view(),
                    }}

                </Suspense>
            </div>
        </div>
    }
}

#[component]
fn BlogShowcaseItem(preview: crate::blog::structures::PostPreview) -> impl IntoView {
    view! {
        <div class="col">
            <a
                href=format!("/blog/{}", preview.slug)
                class="block transform transition-transform hover:scale-105"
            >
                <div class="overflow-hidden rounded-lg bg-white shadow-md dark:bg-stone-800">
                    <img
                        class="h-48 w-full object-cover"
                        src=preview
                            .image_path
                            .unwrap_or(
                                "https://cdn.bootstrapstudio.io/placeholders/1400x800.png"
                                    .to_string(),
                            )
                    />
                    <div class="p-4">
                        <p class="mb-1 text-sm text-blue-600">
                            {match preview.category {
                                Some(c) => c.category_name,
                                None => "Article".to_string(),
                            }}

                        </p>
                        <h4 class="mb-2 text-xl font-semibold">{preview.post_name}</h4>
                        <p class="text-gray-600">{preview.sneak_peak}</p>
                    </div>
                </div>
            </a>
        </div>
    }
}

#[component]
/// Footer for the site
pub fn Footer() -> impl IntoView {
    view! {
        <footer class="text-center shadow-md">
            <div class="mx-auto py-4 text-gray-600 md:py-5">
                <ul class="mb-4 flex justify-center space-x-4">
                    <li>
                        <a class="text-gray-600 transition hover:text-blue-600" href="/">
                            Home
                        </a>
                    </li>
                    <li>
                        <a class="text-gray-600 transition hover:text-blue-600" href="/blog">
                            Blog
                        </a>
                    </li>
                    <li>
                        <a class="text-gray-600 transition hover:text-blue-600" href="/forge">
                            Forge
                        </a>
                    </li>
                </ul>
                <ul class="mb-4 flex justify-center space-x-4">
                    <li>
                        <a
                            href="https://www.facebook.com/profile.php?id=100089118900022"
                            target="_blank"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                class="h-6 w-6 text-gray-600 transition hover:text-blue-600"
                                fill="currentColor"
                                viewBox="0 0 16 16"
                            >
                                <path d="M16 8.049c0-4.446-3.582-8.05-8-8.05C3.58 0-.002 3.603-.002 8.05c0 4.017 2.926 7.347 6.75 7.951v-5.625h-2.03V8.05H6.75V6.275c0-2.017 1.195-3.131 3.022-3.131.876 0 1.791.157 1.791.157v1.98h-1.009c-.993 0-1.303.621-1.303 1.258v1.51h2.218l-.354 2.326H9.25V16c3.824-.604 6.75-3.934 6.75-7.951"></path>
                            </svg>
                        </a>
                    </li>
                    <li>
                        <a href="https://x.com/jkcoxson" target="_blank">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                class="h-6 w-6 text-gray-600 transition hover:text-blue-600"
                                fill="currentColor"
                                viewBox="0 0 16 16"
                            >
                                <path d="M5.026 15c6.038 0 9.341-5.003 9.341-9.334 0-.14 0-.282-.006-.422A6.685 6.685 0 0 0 16 3.542a6.658 6.658 0 0 1-1.889.518 3.301 3.301 0 0 0 1.447-1.817 6.533 6.533 0 0 1-2.087.793A3.286 3.286 0 0 0 7.875 6.03a9.325 9.325 0 0 1-6.767-3.429 3.289 3.289 0 0 0 1.018 4.382A3.323 3.323 0 0 1 .64 6.575v.045a3.288 3.288 0 0 0 2.632 3.218 3.203 3.203 0 0 1-.865.115 3.23 3.23 0 0 1-.614-.057 3.283 3.283 0 0 0 3.067 2.277A6.588 6.588 0 0 1 .78 13.58a6.32 6.32 0 0 1-.78-.045A9.344 9.344 0 0 0 5.026 15"></path>
                            </svg>
                        </a>
                    </li>
                    <li>
                        <a href="https://instagram.com/jkcoxson" target="_blank">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                class="h-6 w-6 text-gray-600 transition hover:text-blue-600"
                                fill="currentColor"
                                viewBox="0 0 16 16"
                            >
                                <path d="M8 0C5.829 0 5.556.01 4.703.048 3.85.088 3.269.222 2.76.42a3.917 3.917 0 0 0-1.417.923A3.927 3.927 0 0 0 .42 2.76C.222 3.268.087 3.85.048 4.7.01 5.555 0 5.827 0 8.001c0 2.172.01 2.444.048 3.297.04.852.174 1.433.372 1.942.205.526.478.972.923 1.417.444.445.89.719 1.416.923.51.198 1.09.333 1.942.372C5.555 15.99 5.827 16 8 16s2.444-.01 3.298-.048c.851-.04 1.434-.174 1.943-.372a3.916 3.916 0 0 0 1.416-.923c.445-.445.718-.891.923-1.417.197-.509.332-1.09.372-1.942C15.99 10.445 16 10.173 16 8s-.01-2.445-.048-3.299c-.04-.851-.175-1.433-.372-1.941a3.926 3.926 0 0 0-.923-1.417A3.911 3.911 0 0 0 13.24.42c-.51-.198-1.092-.333-1.943-.372C10.443.01 10.172 0 7.998 0h.003zm-.717 1.442h.718c2.136 0 2.389.007 3.232.046.78.035 1.204.166 1.486.275.373.145.64.319.92.599.28.28.453.546.598.92.11.281.24.705.275 1.485.039.843.047 1.096.047 3.231s-.008 2.389-.047 3.232c-.035.78-.166 1.203-.275 1.485a2.47 2.47 0 0 1-.599.919c-.28.28-.546.453-.92.598-.28.11-.704.24-1.485.276-.843.038-1.096.047-3.232.047s-2.39-.009-3.233-.047c-.78-.036-1.203-.166-1.485-.276a2.478 2.478 0 0 1-.92-.598 2.48 2.48 0 0 1-.6-.92c-.109-.281-.24-.705-.275-1.485-.038-.843-.046-1.096-.046-3.233 0-2.136.008-2.388.046-3.231.036-.78.166-1.204.276-1.486.145-.373.319-.64.599-.92.28-.28.546-.453.92-.598.282-.11.705-.24 1.485-.276.738-.034 1.024-.044 2.515-.045v.002zm4.988 1.328a.96.96 0 1 0 0 1.92.96.96 0 0 0 0-1.92zm-4.27 1.122a4.109 4.109 0 1 0 0 8.217 4.109 4.109 0 0 0 0-8.217zm0 1.441a2.667 2.667 0 1 1 0 5.334 2.667 2.667 0 0 1 0-5.334"></path>
                            </svg>
                        </a>
                    </li>
                    <li>
                        <a href="https://github.com/jkcoxson" target="_blank">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                class="h-6 w-6 text-gray-600 transition hover:text-blue-600"
                                fill="currentColor"
                                viewBox="0 0 96 96"
                            >
                                <path d="M48.854 0C21.839 0 0 22 0 49.217c0 21.756 13.993 40.172 33.405 46.69 2.427.49 3.316-1.059 3.316-2.362 0-1.141-.08-5.052-.08-9.127-13.59 2.934-16.42-5.867-16.42-5.867-2.184-5.704-5.42-7.17-5.42-7.17-4.448-3.015.324-3.015.324-3.015 4.934.326 7.523 5.052 7.523 5.052 4.367 7.496 11.404 5.378 14.235 4.074.404-3.178 1.699-5.378 3.074-6.6-10.839-1.141-22.243-5.378-22.243-24.283 0-5.378 1.94-9.778 5.014-13.2-.485-1.222-2.184-6.275.486-13.038 0 0 4.125-1.304 13.426 5.052a46.97 46.97 0 0 1 12.214-1.63c4.125 0 8.33.571 12.213 1.63 9.302-6.356 13.427-5.052 13.427-5.052 2.67 6.763.97 11.816.485 13.038 3.155 3.422 5.015 7.822 5.015 13.2 0 18.905-11.404 23.06-22.324 24.283 1.78 1.548 3.316 4.481 3.316 9.126 0 6.6-.08 11.897-.08 13.526 0 1.304.89 2.853 3.316 2.364 19.412-6.52 33.405-24.935 33.405-46.691C97.707 22 75.788 0 48.854 0z"></path>
                            </svg>
                        </a>
                    </li>
                </ul>
                <p class="mb-0">
                    Copyright {" © "} {chrono::Utc::now().date_naive().year()} {" Jackson Coxson"}
                </p>
                <small class="text-gray-500">On to eternal perfection</small>
            </div>
        </footer>
    }
}

#[component]
/// Mobile debug
fn Eruda() -> impl IntoView {
    #[cfg(debug_assertions)]
    view! {
        <Script id="eruda" src="https://cdn.jsdelivr.net/npm/eruda" />
        <div inner_html="<script>eruda.init();</script>"></div>
    }

    #[cfg(not(debug_assertions))]
    view! {}
}
