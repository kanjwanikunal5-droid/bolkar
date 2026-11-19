//! Built-in word packs and starter snippets for the owner (CMA student, finance, AI tools,
//! English + Hinglish). Added 19 Nov 2026 at the owner's request ("add hundreds of words like
//! how I speak"). Every entry is editable or removable in the app.
//!
//! Rules for safety: a term is auto-replaced only when its spelling is distinctive (case,
//! acronym, joined word) and every listed variant is something nobody writes on purpose
//! ("henglish", "speakflow"). Everyday words are hint-only: they help the recogniser and the
//! AI clean-up but are never forced into the text.

use crate::dictionary::Term;
use crate::snippets::Snippet;

fn t(term: &str) -> Term {
    Term::new(term)
}
fn v(term: &str, variants: &[&str]) -> Term {
    Term::with_variants(term, variants)
}
fn h(term: &str) -> Term {
    Term::hint_only(term)
}

/// Cost and management accounting, financial management, tax and law (CMA syllabus).
pub fn study() -> Vec<Term> {
    let mut out = vec![
        v("CMA Inter", &["cma intermediate"]),
        t("CMA Final"),
        t("CMA Foundation"),
        v("ICMAI", &["i c m a i"]),
        v("ICAI", &["i c a i"]),
        v("MCQs", &["mcq's", "m c q s", "mcqs"]),
        v("MCQ", &["m c q"]),
        v("RTP", &["r t p"]),
        v("MTP", &["m t p"]),
        v("P/V ratio", &["pv ratio", "p v ratio"]),
        v("BEP", &["b e p"]),
        v("EOQ", &["e o q"]),
        v("ABC analysis", &["a b c analysis"]),
        v("VED analysis", &["v e d analysis"]),
        v("FIFO", &["fifo", "fi fo"]),
        v("LIFO", &["li fo"]),
        v("ABC", &["a b c"]),
        v("NPV", &["n p v"]),
        v("IRR", &["i r r"]),
        v("MIRR", &["m i r r"]),
        v("WACC", &["w a c c"]),
        v("CAPM", &["c a p m", "cap m"]),
        v("EPS", &["e p s"]),
        v("EBIT-EPS analysis", &["ebit eps analysis"]),
        v("P/E ratio", &["pe ratio", "p e ratio"]),
        v("ROCE", &["r o c e"]),
        v("ROE", &["r o e"]),
        v("DSCR", &["d s c r"]),
        v("CGST", &["c g s t"]),
        v("SGST", &["s g s t"]),
        v("IGST", &["i g s t"]),
        v("UTGST", &["u t g s t"]),
        v("GSTR-1", &["gstr 1", "gstr one"]),
        v("GSTR-3B", &["gstr 3b", "gstr three b"]),
        v("ITC", &["i t c"]),
        v("TCS", &["t c s"]),
        v("ITR", &["i t r"]),
        h("PAN"),
        v("SEBI", &["s e b i"]),
        v("RBI", &["r b i"]),
        v("MCA", &["m c a"]),
        v("ROC", &["r o c"]),
        v("LLP", &["l l p"]),
        v("MOA", &["m o a"]),
        v("AOA", &["a o a"]),
        v("CSR", &["c s r"]),
        v("CARO", &["c a r o"]),
        v("Ind AS", &["ind as", "indas", "ind a s"]),
        v("IFRS", &["i f r s"]),
        v("GAAP", &["g a a p"]),
        v("GDP", &["g d p"]),
        v("CPI", &["c p i"]),
        v("WPI", &["w p i"]),
        v("CRR", &["c r r"]),
        v("SLR", &["s l r"]),
        v("MSME", &["m s m e"]),
        v("e-way bill", &["eway bill", "e way bill"]),
        v("Companies Act, 2013", &["companies act 2013"]),
        v("Income-tax Act", &["income tax act"]),
        v("Halsey plan", &["halsey's plan", "halsey premium plan"]),
        v("Rowan plan", &["rowan's plan"]),
        v(
            "Taylor's differential piece rate",
            &["taylors differential piece rate"],
        ),
        v(
            "Merrick's multiple piece rate",
            &["merricks multiple piece rate"],
        ),
        v("Emerson's efficiency plan", &["emersons efficiency plan"]),
        v("Walter model", &["walters model", "walter's model"]),
        v("Gordon model", &["gordons model", "gordon's model"]),
        v(
            "MM approach",
            &["m m approach", "modigliani miller approach"],
        ),
        v("Kaizen costing", &["kaizen costing", "kaizan costing"]),
    ];
    // Everyday-word phrases: hints for the recogniser and the AI clean-up only.
    for p in [
        "cost accounting",
        "management accounting",
        "cost sheet",
        "prime cost",
        "factory overheads",
        "works cost",
        "cost of production",
        "cost of sales",
        "overhead absorption",
        "overhead apportionment",
        "primary distribution",
        "secondary distribution",
        "repeated distribution method",
        "simultaneous equation method",
        "trial and error method",
        "step ladder method",
        "direct redistribution method",
        "machine hour rate",
        "labour hour rate",
        "single rate method",
        "dual rate method",
        "under-absorption",
        "over-absorption",
        "activity based costing",
        "cost driver",
        "cost pool",
        "marginal costing",
        "absorption costing",
        "contribution",
        "break-even point",
        "margin of safety",
        "angle of incidence",
        "standard costing",
        "standard cost",
        "material cost variance",
        "material price variance",
        "material usage variance",
        "material mix variance",
        "material yield variance",
        "labour cost variance",
        "labour rate variance",
        "labour efficiency variance",
        "idle time variance",
        "overhead cost variance",
        "fixed overhead variance",
        "variable overhead variance",
        "sales variance",
        "sales volume variance",
        "budgetary control",
        "flexible budget",
        "fixed budget",
        "zero based budgeting",
        "cash budget",
        "master budget",
        "performance budgeting",
        "process costing",
        "normal loss",
        "abnormal loss",
        "abnormal gain",
        "equivalent units",
        "joint products",
        "by-products",
        "split-off point",
        "contract costing",
        "work certified",
        "work uncertified",
        "retention money",
        "notional profit",
        "escalation clause",
        "operating costing",
        "service costing",
        "batch costing",
        "job costing",
        "reorder level",
        "maximum stock level",
        "minimum stock level",
        "danger level",
        "lead time",
        "inventory turnover",
        "labour turnover",
        "time keeping",
        "time booking",
        "incentive schemes",
        "reconciliation of cost and financial accounts",
        "uniform costing",
        "inter-firm comparison",
        "target costing",
        "life cycle costing",
        "throughput accounting",
        "transfer pricing",
        "relevant costing",
        "opportunity cost",
        "sunk cost",
        "differential cost",
        "learning curve",
        "cost control",
        "cost reduction",
        "value analysis",
        "payback period",
        "discounted payback period",
        "profitability index",
        "accounting rate of return",
        "capital budgeting",
        "capital structure",
        "cost of capital",
        "cost of equity",
        "cost of debt",
        "cost of preference shares",
        "retained earnings",
        "dividend policy",
        "operating leverage",
        "financial leverage",
        "combined leverage",
        "working capital management",
        "operating cycle",
        "cash conversion cycle",
        "receivables management",
        "factoring",
        "ratio analysis",
        "current ratio",
        "quick ratio",
        "debt-equity ratio",
        "interest coverage ratio",
        "inventory turnover ratio",
        "debtors turnover ratio",
        "fund flow statement",
        "cash flow statement",
        "balance sheet",
        "profit and loss account",
        "trial balance",
        "journal entry",
        "ledger",
        "depreciation",
        "amortisation",
        "accrual",
        "provision",
        "contingent liability",
        "goodwill",
        "consolidated financial statements",
        "holding company",
        "subsidiary company",
        "input tax credit",
        "reverse charge mechanism",
        "place of supply",
        "time of supply",
        "composite supply",
        "mixed supply",
        "tax deducted at source",
        "advance tax",
        "corporate governance",
        "internal audit",
        "cost audit",
        "statutory audit",
        "repo rate",
        "reverse repo rate",
        "monetary policy",
        "fiscal policy",
        "inflation",
        "elasticity of demand",
        "perfect competition",
        "monopoly",
        "oligopoly",
        "monopolistic competition",
        "revision",
        "mock test",
        "syllabus",
        "study material",
    ] {
        out.push(h(p));
    }
    out
}

/// Apps and AI tools the owner uses.
pub fn tools() -> Vec<Term> {
    let mut out = vec![
        v("Claude Code", &["cloud code", "clod code"]),
        v(
            "Claude Cowork",
            &["cloud cowork", "claude co-work", "claude co work"],
        ),
        t("Anthropic"),
        v("OpenAI", &["open ai"]),
        h("Gemini"),
        t("Groq"),
        v("Wispr Flow", &["whisper flow"]),
        t("Typeless"),
        v("Superwhisper", &["super whisper"]),
        h("Perplexity"),
        v("NotebookLM", &["notebook lm", "notebook l m"]),
        h("Notion"),
        t("Canva"),
        t("Figma"),
        v("LinkedIn", &["linked in"]),
        v("WhatsApp", &["whats app", "what's app"]),
        t("Gmail"),
        v("Google Docs", &["google docs"]),
        v("Google Sheets", &["google sheets"]),
        v("Google Meet", &["google meet"]),
        v("YouTube", &["you tube"]),
        t("Instagram"),
        h("Zoom"),
        v("Microsoft Teams", &["ms teams"]),
        h("Outlook"),
        v("PowerPoint", &["power point"]),
        v("Tally Prime", &["tally prime"]),
        h("SAP"),
        t("Zoho"),
        v("API key", &["a p i key", "api key"]),
        v("API", &["a p i"]),
        v("UI", &["u i"]),
        v("UX", &["u x"]),
        v("PWA", &["p w a"]),
        v("APK", &["a p k"]),
        v("MVP", &["m v p"]),
        v("SaaS", &["s a a s"]),
        v("PDF", &["p d f"]),
        v("CSV", &["c s v"]),
        v("URL", &["u r l"]),
        v("AI", &["a i"]),
        v("LLM", &["l l m"]),
        v("GPT", &["g p t"]),
        v("iPhone", &["i phone"]),
        v("Wi-Fi", &["wifi", "wi fi"]),
        v("OTP", &["o t p"]),
        v("UPI", &["u p i"]),
        v("PhonePe", &["phonepe"]),
        v("Paytm", &["pay tm"]),
        v("GPay", &["g pay"]),
    ];
    for p in [
        "prompt",
        "dictation",
        "transcribe",
        "transcription",
        "accuracy",
        "dictionary",
        "snippet",
        "snippets",
        "installer",
        "dashboard",
        "screenshot",
        "deep research",
        "brutal review",
        "Copilot",
        "Windows",
        "Chrome",
        "Whisper",
    ] {
        out.push(h(p));
    }
    out
}

/// Words heard in the owner's dictations (13-19 Nov 2026). No personal names: each user's own
/// name comes from Settings ("Your name"), see [`starter_snippets_for`].
pub fn personal() -> Vec<Term> {
    vec![
        v("Speak Flow", &["speakflow", "speak flo", "speak-flow"]),
        t("Bolkar"),
        v(
            "Hinglish",
            &["henglish", "hinglis", "hing lish", "hinglisch"],
        ),
        h("roll number"),
        h("revision style"),
        h("balanced format"),
        h("hard MCQs"),
        h("Ctrl + Win"),
        h("hands-free"),
    ]
}

/// Starter snippets: say the trigger, get the text. Triggers begin with "insert" so they are
/// never said by accident. All are editable in the app (Snippets).
/// Bodies may contain `{name}`; use [`starter_snippets_for`] to fill it in.
pub fn starter_snippets() -> Vec<Snippet> {
    let s = |trigger: &str, body: &str| Snippet {
        trigger: trigger.to_string(),
        body: body.to_string(),
    };
    vec![
        // Study prompts (CMA).
        s("insert revision prompt", "Explain this topic in a revision-style format. I have already learned it and I am revising now. Keep it balanced: not too short and not too detailed. Cover the key concepts, formulas and common exam traps, then give me 5 hard MCQs with answers and short explanations."),
        s("insert mcq prompt", "Give me 10 hard MCQs on this topic at CMA exam level, with four options each. Show the answers and a short explanation for each at the end, not next to the questions."),
        s("insert numerical prompt", "Give me one exam-level numerical problem on this topic, then solve it step by step with the working shown clearly, and point out where students usually make mistakes."),
        s("insert example prompt", "Give me one more practical business example of this, with simple numbers, so I can understand it properly."),
        s("insert explain simply", "Explain this in the simplest way possible, as if I am learning it for the first time, with one easy example."),
        s("insert difference prompt", "Explain the difference between these in a clear table, then give one line on when each is used."),
        s("insert formula sheet", "Make a short formula sheet for this chapter: every formula with what each term means and one line on when to use it."),
        s("insert summary prompt", "Summarise this chapter in short revision notes: key definitions, formulas, important points and exam tips."),
        s("insert flashcards prompt", "Turn this topic into 15 question-and-answer flashcards for quick revision."),
        s("insert case study prompt", "Give me a short case study on this topic with three questions, then the model answers."),
        s("insert check my answer", "Check my answer below like a strict CMA examiner. Tell me what is right, what is wrong, what is missing and the marks you would give out of 10."),
        s("insert study plan prompt", "Make a day-by-day revision plan for the topics below, with time for practice questions and one full mock test at the end."),
        s("insert mnemonic prompt", "Give me an easy way to remember this (a mnemonic or a short story)."),
        s("insert past paper prompt", "Show me how this topic is usually asked in CMA exams, with the types of questions and how to answer each."),
        s("insert costing example", "Explain with a cost sheet example: prime cost, factory overheads, cost of production and cost of sales, step by step."),
        s("insert variance prompt", "Explain all the related variances with formulas, one solved example, and how to check that the variances add up to the total variance."),
        // Working with Claude / ChatGPT.
        s("insert do it properly", "Understand everything properly before you start, plan it, test it, and do not make any mistakes. If something is unclear, tell me instead of guessing."),
        s("insert brutal review", "Review this brutally and honestly. Rate it out of 10, list every problem in order of importance, and give me concrete fixes for each."),
        s("insert deep research prompt", "Do deep research on this: compare the options, use reliable sources, and give me a clear recommendation with the reasons."),
        s("insert step by step", "Explain it step by step, one step at a time, and wait for me before you move to the next step."),
        s("insert table format", "Give the answer in a clear table."),
        s("insert bullet format", "Give the answer in short bullet points."),
        s("insert keep it short", "Keep the answer short and to the point."),
        s("insert more detail", "Go into more detail on this, with examples."),
        s("insert simple english", "Use simple English, short sentences and no jargon."),
        s("insert hinglish answer", "Answer in Hinglish (Roman letters), the way we chat on WhatsApp."),
        s("insert continue prompt", "Continue from where you stopped."),
        s("insert fix the code", "Find the bug in this, explain the cause in one line, and give me the corrected version."),
        s("insert improve this", "Improve this: make it clearer and more professional, but keep my meaning and my words where possible."),
        s("insert proofread this", "Proofread this. Fix only spelling, grammar and punctuation; do not change the meaning or the tone."),
        s("insert rewrite formal", "Rewrite this in a polite, formal tone suitable for an email."),
        s("insert rewrite casual", "Rewrite this in a friendly, casual tone for a WhatsApp message."),
        s("insert linkedin post prompt", "Write a LinkedIn post about this: a strong first line, short paragraphs, a personal angle, and three relevant hashtags at the end."),
        s("insert caption prompt", "Write three short, catchy captions for this post."),
        s("insert email prompt", "Write a short, polite email about the points below."),
        s("insert reply prompt", "Draft a reply to this message: polite, clear and short."),
        s("insert message prompt", "Draft a message for this person, short and friendly."),
        s("insert pros and cons", "List the pros and cons in a table, then give your recommendation."),
        s("insert explain like teacher", "Explain this like a good teacher: start with the idea, then an example, then a quick question to check my understanding."),
        s("insert speak flow prompt", "This is about my app Speak Flow (voice dictation for English and Hinglish on Windows). Keep that context in mind."),
        // Messages, English.
        s("insert on my way", "On my way, I will be there soon."),
        s("insert running late", "Sorry, I am running a little late. I will be there in 15 minutes."),
        s("insert call you later", "I am busy right now. I will call you later."),
        s("insert in a meeting", "I am in a meeting right now. I will get back to you soon."),
        s("insert studying now", "I am studying right now. I will message you after some time."),
        s("insert thank you message", "Thank you so much, I really appreciate it."),
        s("insert sorry message", "Sorry for the delay in replying."),
        s("insert noted message", "Noted, thank you. I will take care of it."),
        s("insert sounds good", "Sounds good, let's do that."),
        s("insert talk tomorrow", "Let's talk tomorrow. Good night!"),
        s("insert good morning", "Good morning! Hope you have a great day."),
        s("insert happy birthday", "Happy birthday! Wishing you a wonderful year ahead, full of happiness and success."),
        s("insert congratulations", "Congratulations! Very well deserved, all the best for what comes next."),
        s("insert get well soon", "Get well soon! Take care and rest well."),
        s("insert festival wishes", "Wishing you and your family a very happy and prosperous festival!"),
        s("insert send the details", "Could you please send me the details when you get a moment?"),
        s("insert share the notes", "Could you please share the notes for this chapter?"),
        s("insert let me check", "Let me check and get back to you."),
        s("insert will do", "Sure, I will do it."),
        s("insert no problem", "No problem at all."),
        s("insert please confirm", "Please confirm once it is done."),
        s("insert reminder message", "Just a gentle reminder about this. Please let me know the update."),
        s("insert follow up", "Following up on my earlier message. Please let me know when you get a chance."),
        s("insert meeting request", "Could we schedule a quick call this week to discuss this? Please let me know a time that suits you."),
        s("insert availability", "I am available tomorrow after 4 PM. Does that work for you?"),
        // Messages, Hinglish.
        s("insert raste mein", "Main raste mein hoon, thodi der mein pahunch raha hoon."),
        s("insert baad mein call", "Abhi thoda busy hoon, baad mein call karta hoon."),
        s("insert padhai chal rahi", "Abhi padhai kar raha hoon, thodi der baad baat karte hain."),
        s("insert theek hai bhai", "Theek hai bhai, ho jayega. Tension mat le."),
        s("insert kal milte hain", "Kal milte hain, phir plan banate hain."),
        s("insert notes bhej do", "Bhai is chapter ke notes bhej de please."),
        s("insert pahunch gaya", "Main ghar pahunch gaya hoon."),
        s("insert khana kha liya", "Haan, khana kha liya. Tum batao?"),
        s("insert shukriya bhai", "Thanks bhai, bahut help ho gayi."),
        s("insert sorry late", "Sorry yaar, late ho gaya. Abhi dekhta hoon."),
        s("insert kya scene hai", "Kya scene hai aaj ka? Kuch plan hai kya?"),
        s("insert ghar pe hoon", "Main ghar pe hoon, aa jao."),
        s("insert time bata do", "Time bata do, main aa jaunga."),
        s("insert ho gaya kaam", "Kaam ho gaya hai, check kar lo."),
        s("insert janamdin mubarak", "Janamdin ki bahut bahut badhai! Khush raho, aage badhte raho."),
        // Emails.
        s("insert email greeting", "Dear Sir/Madam,\n\nI hope you are doing well."),
        s("insert email closing", "Thank you for your time and consideration.\n\nBest regards,\n{name}"),
        s("insert my signature", "Best regards,\n{name}"),
        s("insert leave request", "I would like to request leave on the dates below due to my exams. Please let me know if you need anything else from me."),
        s("insert assignment submission", "Please find my assignment attached. Kindly let me know if any changes are needed."),
        s("insert query email", "I have a query regarding the topic below. Could you please guide me?"),
        s("insert thank you email", "Thank you for your quick response and support. I really appreciate it."),
        s("insert apology email", "I apologise for the inconvenience caused and I will make sure it does not happen again."),
        s("insert follow up email", "I am writing to follow up on my previous email. I would be grateful for an update when convenient."),
        s("insert meeting confirmation", "This is to confirm our meeting as discussed. Please let me know if anything changes."),
        s("insert attached document", "Please find the document attached for your reference."),
        s("insert my name", "{name}"),
    ]
}

/// A name typed all in capitals ("ASHA RAO") is written "Asha Rao" in a signature; any other
/// spelling is kept exactly as typed.
fn tidy_name(name: &str) -> String {
    let name = name.trim();
    let letters: Vec<char> = name.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() < 2 || letters.iter().any(|c| c.is_lowercase()) {
        return name.to_string();
    }
    name.split(' ')
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(f) => f
                    .to_uppercase()
                    .chain(cs.flat_map(char::to_lowercase))
                    .collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// The starter snippets with `{name}` replaced by the user's name. Snippets that need a name are
/// left out when no name is set, so nobody gets someone else's name.
pub fn starter_snippets_for(name: &str) -> Vec<Snippet> {
    let name = tidy_name(name);
    let name = name.as_str();
    starter_snippets()
        .into_iter()
        .filter(|s| !name.is_empty() || !s.body.contains("{name}"))
        .map(|s| Snippet {
            body: s.body.replace("{name}", name),
            ..s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::Dictionary;

    #[test]
    fn packs_are_large_unique_and_safe() {
        let mut all = study();
        all.extend(tools());
        all.extend(personal());
        assert!(all.len() >= 300, "{} terms", all.len());
        let mut seen = std::collections::HashSet::new();
        for term in &all {
            assert!(
                seen.insert(term.term.to_lowercase()),
                "duplicate {}",
                term.term
            );
        }
        let d = Dictionary::new(all);
        let mut c = Vec::new();
        assert_eq!(
            d.apply(
                "the henglish mode of speakflow uses groq api and the mcqs",
                &mut c
            ),
            "the Hinglish mode of Speak Flow uses Groq API and the MCQs"
        );
        // Everyday words stay as they are.
        for s in [
            "move the cursor down",
            "the cloud is grey",
            "i will write a word",
        ] {
            assert_eq!(d.apply(s, &mut Vec::new()), s);
        }
    }

    #[test]
    fn snippets_have_safe_unique_triggers() {
        let s = starter_snippets();
        assert!(s.len() >= 90, "{} snippets", s.len());
        let mut seen = std::collections::HashSet::new();
        for x in &s {
            assert!(x.trigger.starts_with("insert "), "{}", x.trigger);
            assert!(x.trigger.split_whitespace().count() >= 2);
            assert!(seen.insert(x.trigger.clone()), "duplicate {}", x.trigger);
            assert!(!x.body.trim().is_empty());
        }
        // The user's own name is filled in; without a name, those snippets are left out.
        let named = starter_snippets_for(" Asha Rao ");
        let sig = named
            .iter()
            .find(|x| x.trigger == "insert my signature")
            .unwrap();
        assert_eq!(
            sig.body,
            "Best regards,
Asha Rao"
        );
        assert!(named.iter().all(|x| !x.body.contains("{name}")));
        let unnamed = starter_snippets_for("");
        assert!(unnamed.iter().all(|x| !x.body.contains("{name}")));
        assert!(!unnamed.iter().any(|x| x.trigger == "insert my name"));
        assert_eq!(named.len(), s.len());
        let caps = starter_snippets_for("ASHA RAO");
        assert!(caps.iter().any(|x| x.body == "Asha Rao"));
        let mixed = starter_snippets_for("Asha deSouza");
        assert!(mixed.iter().any(|x| x.body == "Asha deSouza"));
    }
}

#[cfg(test)]
mod overlap {
    #[test]
    fn new_packs_do_not_repeat_the_old_ones() {
        let old: std::collections::HashSet<String> = ["finance", "tech"]
            .iter()
            .flat_map(|p| crate::dictionary::pack(p))
            .map(|t| t.term.to_lowercase())
            .collect();
        for p in ["study", "tools", "personal"] {
            for t in crate::dictionary::pack(p) {
                assert!(
                    !old.contains(&t.term.to_lowercase()),
                    "{} is already in an older pack",
                    t.term
                );
            }
        }
    }
}
