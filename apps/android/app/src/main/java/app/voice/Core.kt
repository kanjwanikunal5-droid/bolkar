package app.voice

/**
 * The shared Rust clean-up pipeline (crates/android), the same code the desktop and web apps
 * run, so the golden fixtures in spec/fixtures hold here too.
 */
object Core {
    init {
        System.loadLibrary("app_android")
    }

    @JvmStatic external fun clean(raw: String, level: String, spoken: Boolean, vocabulary: String): String

    /** JSON {"system": ..., "user": ...}. */
    @JvmStatic external fun cleanupRequest(
        raw: String,
        level: String,
        spoken: Boolean,
        vocabulary: String,
        language: String,
    ): String

    /** JSON {"ok": true, "text"} or {"ok": false, "why", "text": rules result}. */
    @JvmStatic external fun acceptCleanup(
        answer: String,
        raw: String,
        level: String,
        spoken: Boolean,
        vocabulary: String,
    ): String

    @JvmStatic external fun hasDevanagari(text: String): Boolean
    @JvmStatic external fun transliteratePrompt(): String

    /** The Roman text, or "" when the answer is rejected. */
    @JvmStatic external fun acceptTransliteration(answer: String, source: String): String
    @JvmStatic external fun polishPrompt(): String

    /** The polished text, or "" when the answer is rejected. */
    @JvmStatic external fun acceptPolish(answer: String, source: String): String
    @JvmStatic external fun romanHinglishExample(): String

    /** Offline Hinglish spelling rules (bahut, karunga, ...). */
    @JvmStatic external fun hinglishSpelling(text: String): String
    @JvmStatic external fun recogniserHint(vocabulary: String): String
    @JvmStatic external fun defaultTerms(): String
    @JvmStatic external fun appName(): String
}
