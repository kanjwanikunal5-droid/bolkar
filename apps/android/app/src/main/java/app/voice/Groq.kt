package app.voice

import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL

/** Groq's OpenAI-compatible API, called with the user's own key. */
object Groq {
    private const val BASE = "https://api.groq.com/openai/v1"
    const val ASR_MODEL = "whisper-large-v3"
    const val CHAT_MODEL = "qwen/qwen3.8-27b"

    class ApiError(message: String) : Exception(message)

    /** Speech to text. [language] null lets Whisper detect it. */
    fun transcribe(key: String, wav: ByteArray, language: String?, prompt: String?): String {
        val boundary = "----voice" + System.nanoTime()
        val body = ByteArrayOutputStream()
        fun field(name: String, value: String) {
            body.write("--$boundary\r\nContent-Disposition: form-data; name=\"$name\"\r\n\r\n$value\r\n".toByteArray())
        }
        field("model", ASR_MODEL)
        field("temperature", "0")
        field("response_format", "json")
        language?.let { field("language", it) }
        prompt?.takeIf { it.isNotBlank() }?.let { field("prompt", it) }
        body.write(
            ("--$boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n" +
                "Content-Type: audio/wav\r\n\r\n").toByteArray(),
        )
        body.write(wav)
        body.write("\r\n--$boundary--\r\n".toByteArray())
        val reply = post("$BASE/audio/transcriptions", key, "multipart/form-data; boundary=$boundary", body.toByteArray(), 30_000)
        return JSONObject(reply).optString("text").trim()
    }

    /** One chat completion; returns the answer text. */
    fun chat(key: String, system: String, user: String, timeoutMs: Int = 6_000): String {
        val body = JSONObject()
            .put("model", CHAT_MODEL)
            .put("temperature", 0)
            // Qwen 3 thinks before answering by default; off for speed, reasoning kept out.
            .put("reasoning_effort", "none")
            .put("reasoning_format", "hidden")
            .put(
                "messages",
                JSONArray()
                    .put(JSONObject().put("role", "system").put("content", system))
                    .put(JSONObject().put("role", "user").put("content", user)),
            )
        val reply = post("$BASE/chat/completions", key, "application/json", body.toString().toByteArray(), timeoutMs)
        return JSONObject(reply).getJSONArray("choices").getJSONObject(0).getJSONObject("message").optString("content")
    }

    private fun post(url: String, key: String, type: String, payload: ByteArray, timeoutMs: Int): String {
        val c = URL(url).openConnection() as HttpURLConnection
        try {
            c.requestMethod = "POST"
            c.connectTimeout = timeoutMs
            c.readTimeout = timeoutMs
            c.doOutput = true
            c.setRequestProperty("Authorization", "Bearer $key")
            c.setRequestProperty("Content-Type", type)
            c.outputStream.use { it.write(payload) }
            val code = c.responseCode
            if (code !in 200..299) {
                val detail = c.errorStream?.bufferedReader()?.use { it.readText() }.orEmpty()
                throw ApiError(
                    when (code) {
                        401 -> "Groq did not accept the key (401). Check it in the app."
                        429 -> "Groq's free limit was reached (429). Wait a minute and try again."
                        else -> "Groq error $code ${detail.take(120)}"
                    },
                )
            }
            return c.inputStream.bufferedReader().use { it.readText() }
        } finally {
            c.disconnect()
        }
    }
}
