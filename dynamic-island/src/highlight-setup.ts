import hljs from "highlight.js/lib/core";

import javascript from "highlight.js/lib/languages/javascript";

import typescript from "highlight.js/lib/languages/typescript";

import python from "highlight.js/lib/languages/python";

import css from "highlight.js/lib/languages/css";

import xml from "highlight.js/lib/languages/xml";

import json from "highlight.js/lib/languages/json";

import bash from "highlight.js/lib/languages/bash";

import rust from "highlight.js/lib/languages/rust";

import java from "highlight.js/lib/languages/java";

import cpp from "highlight.js/lib/languages/cpp";

import sql from "highlight.js/lib/languages/sql";

import markdown from "highlight.js/lib/languages/markdown";



hljs.registerLanguage("javascript", javascript);

hljs.registerLanguage("js", javascript);

hljs.registerLanguage("typescript", typescript);

hljs.registerLanguage("ts", typescript);

hljs.registerLanguage("python", python);

hljs.registerLanguage("py", python);

hljs.registerLanguage("css", css);

hljs.registerLanguage("html", xml);

hljs.registerLanguage("xml", xml);

hljs.registerLanguage("json", json);

hljs.registerLanguage("bash", bash);

hljs.registerLanguage("sh", bash);

hljs.registerLanguage("rust", rust);

hljs.registerLanguage("rs", rust);

hljs.registerLanguage("java", java);

hljs.registerLanguage("cpp", cpp);

hljs.registerLanguage("c", cpp);

hljs.registerLanguage("sql", sql);

hljs.registerLanguage("markdown", markdown);

hljs.registerLanguage("md", markdown);

export { hljs };
