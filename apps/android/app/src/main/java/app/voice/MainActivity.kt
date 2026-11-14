package app.voice

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Typeface
import android.os.Bundle
import android.provider.Settings
import android.text.InputType
import android.util.TypedValue
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.RadioGroup
import android.widget.ScrollView
import android.widget.TextView

/** Set-up and settings, in large text: turn the keyboard on, allow the mic, choose engine. */
class MainActivity : Activity() {
    private lateinit var prefs: Prefs
    private lateinit var secrets: SecretStore
    private lateinit var keyboardState: TextView
    private lateinit var micState: TextView
    private lateinit var keyState: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = Prefs(this)
        secrets = SecretStore(this)
        val name = getString(R.string.app_name)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(20), dp(24), dp(20), dp(40))
        }
        setContentView(ScrollView(this).apply { addView(col) })

        col.addView(text(name, 30f, bold = true))
        col.addView(text("Voice typing for English and Hinglish in any app.", 20f))

        col.addView(heading("1. Turn on the keyboard"))
        keyboardState = text("", 18f)
        col.addView(keyboardState)
        col.addView(button("Open keyboard settings") { startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS)) })
        col.addView(button("Choose $name as the keyboard") {
            (getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager).showInputMethodPicker()
        })

        col.addView(heading("2. Allow the microphone"))
        micState = text("", 18f)
        col.addView(micState)
        col.addView(button("Allow microphone") { requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO), 1) })

        col.addView(heading("3. Speech recognition"))
        val engines = RadioGroup(this)
        val phone = radio("Phone's own recogniser (no key needed). English works best; Hinglish comes out in Hindi letters unless AI text steps are on.")
        val groq = radio("Groq Whisper with your own free key (best for Hinglish). Your recorded voice is sent to Groq.")
        engines.addView(phone)
        engines.addView(groq)
        engines.check(if (prefs.engine == "groq") groq.id else phone.id)
        engines.setOnCheckedChangeListener { _, id -> prefs.engine = if (id == groq.id) "groq" else "phone" }
        col.addView(engines)
        col.addView(check("I agree that my dictation audio is sent to Groq for speech recognition.", prefs.audioConsent) {
            prefs.audioConsent = it
        })

        col.addView(heading("4. Groq key (optional)"))
        col.addView(text("Create a free key at console.groq.com/keys and paste it here. It is stored encrypted on this phone (Android Keystore) and only sent to Groq.", 18f))
        keyState = text("", 18f)
        col.addView(keyState)
        val keyField = EditText(this).apply {
            hint = "Paste your Groq key"
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 20f)
        }
        col.addView(keyField)
        col.addView(button("Save key") {
            val k = keyField.text.toString().trim()
            if (k.isNotEmpty()) {
                secrets.save(k)
                keyField.setText("")
            }
            refresh()
        })
        col.addView(button("Remove key") {
            secrets.clear()
            refresh()
        })

        col.addView(heading("5. AI text steps (optional)"))
        col.addView(check(
            "I agree that the text of each dictation is sent to Groq to fix Hinglish spelling and tidy punctuation. Answers are checked; anything that changes numbers or adds words is thrown away.",
            prefs.textConsent,
        ) { prefs.textConsent = it })

        col.addView(heading("6. Language"))
        val langs = RadioGroup(this)
        val options = listOf("en-hinglish" to "English + Hinglish (decided each time)", "en" to "English only", "hinglish" to "Hinglish only")
        options.forEach { (value, label) ->
            val r = radio(label)
            langs.addView(r)
            if (prefs.language == value) langs.check(r.id)
            r.setOnClickListener { prefs.language = value }
        }
        col.addView(langs)

        col.addView(heading("7. Try it"))
        col.addView(EditText(this).apply {
            hint = "Tap here, then tap the microphone key"
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 20f)
            minLines = 3
        })
        col.addView(text("Nothing is recorded unless you tap the microphone. Recordings stay in memory and are discarded after typing.", 16f))
    }

    override fun onResume() {
        super.onResume()
        refresh()
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        refresh()
    }

    private fun refresh() {
        val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
        val enabled = imm.enabledInputMethodList.any { it.packageName == packageName }
        keyboardState.text = if (enabled) "✓ Keyboard is turned on." else "Keyboard is off. Turn it on in keyboard settings."
        val mic = checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
        micState.text = if (mic) "✓ Microphone allowed." else "Microphone not allowed yet."
        keyState.text = if (secrets.load() != null) "✓ A key is saved." else "No key saved."
    }

    // ---- large, high-contrast widgets ----

    private fun text(s: String, sp: Float, bold: Boolean = false) = TextView(this).apply {
        text = s
        setTextSize(TypedValue.COMPLEX_UNIT_SP, sp)
        setTextColor(getColor(R.color.ink))
        if (bold) typeface = Typeface.DEFAULT_BOLD
        setPadding(0, dp(4), 0, dp(4))
    }

    private fun heading(s: String) = text(s, 22f, bold = true).apply { setPadding(0, dp(24), 0, dp(4)) }

    private fun button(label: String, action: () -> Unit) = Button(this).apply {
        text = label
        isAllCaps = false
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 20f)
        minHeight = dp(56)
        setOnClickListener { action() }
    }

    private fun radio(label: String) = RadioButton(this).apply {
        id = View.generateViewId()
        text = label
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
        setPadding(dp(4), dp(8), 0, dp(8))
    }

    private fun check(label: String, value: Boolean, onChange: (Boolean) -> Unit) = CheckBox(this).apply {
        text = label
        isChecked = value
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
        setPadding(dp(4), dp(8), 0, dp(8))
        setOnCheckedChangeListener { _, v -> onChange(v) }
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()
}
