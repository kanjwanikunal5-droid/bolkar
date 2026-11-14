package app.voice

import org.json.JSONObject

/**
 * What happens after the microphone stops: recognition (Groq) and the text steps shared with
 * the desktop app: Devanagari to Roman, Hinglish spelling, clean-up. Every AI answer is checked
 * by the Rust core; a rejected answer is dropped and the text before it is kept.
 */
class Dictation(private val prefs: Prefs, private val secrets: SecretStore) {
    data class Result(val raw: String, val text: String, val notes: List<String>)

    /** Speech to text with the user's Groq key (only after they agreed to send audio). */
    fun recogniseWithGroq(pcm: ByteArray): String {
        val key = secrets.load() ?: throw Groq.ApiError("Add your Groq key in the app first.")
        if (!prefs.audioConsent) throw Groq.ApiError("Allow sending audio to Groq in the app first.")
        val (language, prompt) = when (prefs.language) {
            "en" -> "en" to Core.recogniserHint(prefs.vocabulary)
            // Hindi recognition steered to Roman letters by an example (measured on the
            // owner's voice notes with the desktop engine; see docs/STATUS.md).
            "hinglish" -> "hi" to Core.romanHinglishExample()
            // English or Hinglish. Whisper's own detection picks English for Hinglish that is
            // mostly English and then translates the Hindi part (measured on the desktop), so
            // with AI text steps on, Hindi recognition is used: it keeps English words in
            // English and the Hindi letters are converted to Roman afterwards.
            else -> if (prefs.textConsent) "hi" to Core.romanHinglishExample() else null to Core.recogniserHint(prefs.vocabulary)
        }
        return Groq.transcribe(key, Recorder.wav(pcm), language, prompt)
    }

    /** The text steps. [hinglish] is true when the Hinglish route was used. */
    fun finish(raw: String, hinglish: Boolean): Result {
        val notes = mutableListOf<String>()
        var text = raw.trim()
        if (text.isEmpty()) return Result(raw, "", notes)
        val key = if (prefs.textConsent) secrets.load() else null

        // Hinglish is typed in Roman letters, as in WhatsApp.
        if (Core.hasDevanagari(text) && prefs.language != "en") {
            if (key == null) {
                notes += "Hindi letters kept: allow AI text steps in the app to convert them."
            } else {
                val answer = runCatching {
                    Groq.chat(key, Core.transliteratePrompt(), JSONObject().put("text", text).toString())
                }
                val roman = answer.getOrNull()?.let { Core.acceptTransliteration(it, text) }.orEmpty()
                if (roman.isNotEmpty()) text = roman else notes += "Script conversion skipped."
            }
        }

        // Common WhatsApp spellings and obvious mis-hearings.
        val hinglishText = hinglish || (prefs.language != "en" && looksHinglish(text))
        if (hinglishText && !Core.hasDevanagari(text)) text = Core.hinglishSpelling(text)
        if (hinglishText && key != null && !Core.hasDevanagari(text)) {
            val answer = runCatching {
                Groq.chat(key, Core.polishPrompt(), JSONObject().put("text", text).toString())
            }
            val polished = answer.getOrNull()?.let { Core.acceptPolish(it, text) }.orEmpty()
            if (polished.isNotEmpty()) text = polished else notes += "Spelling polish skipped."
        }

        // Clean-up: AI when allowed (checked against inventions), otherwise the rules.
        val level = prefs.level
        val vocabulary = prefs.vocabulary
        val final = if (key != null && level != "LITERAL") {
            val language = if (hinglishText) "hi-en" else "en"
            runCatching {
                val req = JSONObject(Core.cleanupRequest(text, level, true, vocabulary, language))
                val answer = Groq.chat(key, req.getString("system"), req.getString("user"))
                val verdict = JSONObject(Core.acceptCleanup(answer, text, level, true, vocabulary))
                if (!verdict.optBoolean("ok")) notes += "AI clean-up answer rejected; rules used."
                verdict.getString("text")
            }.getOrElse {
                notes += "AI clean-up unavailable; rules used."
                Core.clean(text, level, true, vocabulary)
            }
        } else {
            Core.clean(text, level, true, vocabulary)
        }
        return Result(raw, final, notes)
    }

    companion object {
        private val HINDI_MARKERS = setOf(
            "hai", "hain", "hoon", "hu", "nahi", "nahin", "kya", "kyun", "mujhe", "tujhe", "mera",
            "mere", "meri", "tera", "raha", "rahi", "karunga", "karna", "kar", "bhai", "yaar",
            "aaj", "kal", "abhi", "baad", "mein", "toh", "bahut", "accha", "acha", "theek", "thik",
        )

        /** Two or more common Hindi words written in Roman letters. */
        fun looksHinglish(text: String): Boolean =
            text.lowercase().split(Regex("[^a-z]+")).filter { it in HINDI_MARKERS }.toSet().size >= 2
    }
}
