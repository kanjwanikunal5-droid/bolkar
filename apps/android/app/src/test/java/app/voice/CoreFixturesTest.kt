package app.voice

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Runs the golden clean-up fixtures (spec/fixtures) through the JNI bridge on the host JVM,
 * so the Android build is held to the same cases as the desktop and web apps.
 * Needs HOST_LIB_DIR pointing at the host build of crates/android (the CI job sets it).
 */
class CoreFixturesTest {
    @Test
    fun goldenFixturesPassThroughTheBridge() {
        val dir = File(System.getProperty("fixtures.dir") ?: error("fixtures.dir not set"))
        var total = 0
        val failures = mutableListOf<String>()
        var mustPassFailures = 0
        dir.listFiles { f -> f.extension == "json" }!!.sorted().forEach { file ->
            val json = JSONObject(file.readText())
            val defaults = json.getJSONObject("defaults")
            val cases = json.getJSONArray("cases")
            for (i in 0 until cases.length()) {
                val c = cases.getJSONObject(i)
                total++
                val vocabulary = JSONObject()
                    .put("terms", c.optJSONArray("dictionary") ?: org.json.JSONArray())
                    .put("snippets", c.optJSONArray("snippets") ?: org.json.JSONArray())
                    .toString()
                val got = Core.clean(
                    c.getString("input"),
                    c.optString("level", defaults.getString("level")),
                    c.optBoolean("spoken_commands", defaults.getBoolean("spoken_commands")),
                    vocabulary,
                )
                if (got != c.getString("expected")) {
                    if (c.optBoolean("must_pass")) mustPassFailures++
                    failures += "[${c.getString("id")}] got ${got}"
                }
            }
        }
        val rate = (total - failures.size) * 100.0 / total
        println("golden fixtures via JNI: ${total - failures.size}/$total (${"%.1f".format(rate)}%)")
        failures.forEach { println("FAILED $it") }
        assertTrue("no fixtures found", total > 0)
        assertEquals("a must-pass golden example failed", 0, mustPassFailures)
        assertTrue("pass rate $rate% is below 95%", rate >= 95.0)
    }

    @Test
    fun hinglishChecksRejectRewrites() {
        assertTrue(Core.romanHinglishExample().startsWith("Haan bhai"))
        assertEquals(
            "Main bahut busy hoon.",
            Core.acceptPolish("{\"text\":\"Main bahut busy hoon.\"}", "Main baut busy hoon."),
        )
        assertEquals("", Core.acceptPolish("{\"text\":\"I am very busy.\"}", "Main baut busy hoon."))
        assertTrue(Core.hasDevanagari("मैं"))
        assertFalse(Core.hasDevanagari("main"))
        val bad = JSONObject(Core.acceptCleanup("{\"text\":\"Let's meet at 4 PM.\"}", "lets meet at 3 PM", "LIGHT", true, "{}"))
        assertFalse(bad.getBoolean("ok"))
    }

    @Test
    fun hinglishIsSpottedInRomanText() {
        assertTrue(Dictation.looksHinglish("Bhai main abhi busy hoon"))
        assertFalse(Dictation.looksHinglish("Please send the report by Friday"))
    }
}
