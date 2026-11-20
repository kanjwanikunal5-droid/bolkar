package app.voice

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.util.TypedValue
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import java.util.concurrent.Executors

/**
 * The voice keyboard: one large microphone key plus space, delete, enter, language and
 * "switch keyboard". Tap the microphone, speak, tap again; the text is typed where the
 * cursor is.
 */
class VoiceKeyboard : InputMethodService() {
    private lateinit var prefs: Prefs
    private lateinit var secrets: SecretStore
    private val main = Handler(Looper.getMainLooper())
    private val worker = Executors.newSingleThreadExecutor()
    private val recorder = Recorder()
    private var recognizer: SpeechRecognizer? = null

    private lateinit var status: TextView
    private lateinit var levelBar: ProgressBar
    private lateinit var mic: Button
    private lateinit var languageKey: Button

    private enum class State { IDLE, LISTENING, WORKING }
    private var state = State.IDLE

    override fun onCreate() {
        super.onCreate()
        prefs = Prefs(this)
        secrets = SecretStore(this)
    }

    override fun onCreateInputView(): View {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(getColor(R.color.keyboard))
            setPadding(dp(8), dp(8), dp(8), dp(8))
        }
        status = TextView(this).apply {
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
            setTextColor(getColor(R.color.ink))
            gravity = Gravity.CENTER
            setPadding(0, dp(4), 0, dp(4))
        }
        levelBar = ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal).apply {
            max = 100
            progress = 0
        }
        mic = Button(this).apply {
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 24f)
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.WHITE)
            isAllCaps = false
            setOnClickListener { onMic() }
        }
        root.addView(status, LinearLayout.LayoutParams(-1, -2))
        root.addView(levelBar, LinearLayout.LayoutParams(-1, dp(8)))
        root.addView(mic, LinearLayout.LayoutParams(-1, dp(84)).apply { topMargin = dp(8) })

        val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        fun key(label: String, weight: Float, description: String, action: () -> Unit) =
            Button(this).apply {
                text = label
                contentDescription = description
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 20f)
                setTextColor(getColor(R.color.ink))
                isAllCaps = false
                background = rounded(getColor(R.color.key))
                setOnClickListener { action() }
                row.addView(this, LinearLayout.LayoutParams(0, dp(60), weight).apply { setMargins(dp(3), dp(6), dp(3), 0) })
            }
        key("🌐", 1f, "Switch keyboard") { switchKeyboard() }
        languageKey = key("", 1.6f, "Language") { cycleLanguage() }
        key("space", 2.6f, "Space") { currentInputConnection?.commitText(" ", 1) }
        key("⌫", 1.2f, "Delete") { delete() }.setOnLongClickListener { deleteWord(); true }
        key("⏎", 1.2f, "Enter") { enter() }
        root.addView(row)
        showIdle()
        return root
    }

    override fun onStartInputView(info: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        if (state == State.IDLE) showIdle()
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        // Leaving the text field ends any recording; nothing recorded is kept.
        if (state == State.LISTENING) cancel()
        super.onFinishInputView(finishingInput)
    }

    override fun onDestroy() {
        cancel()
        worker.shutdownNow()
        super.onDestroy()
    }

    // ---- microphone ----

    private fun onMic() {
        when (state) {
            State.IDLE -> start()
            State.LISTENING -> stop()
            State.WORKING -> Unit
        }
    }

    private fun start() {
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            showMessage("Open the ${getString(R.string.app_name)} app and allow the microphone.", error = true)
            return
        }
        if (prefs.engine == "groq") {
            if (secrets.load() == null || !prefs.audioConsent) {
                showMessage("Groq is chosen but not set up. Open the app to add your key.", error = true)
                return
            }
            if (!recorder.start()) {
                showMessage("The microphone is busy or unavailable.", error = true)
                return
            }
            setListening()
            pollLevel()
        } else {
            startPhoneRecognizer()
        }
    }

    private fun stop() {
        if (prefs.engine == "groq") {
            val pcm = recorder.stop()
            recorder.stopProblem?.let { android.util.Log.w("SpeakFlow", it) }
            if (Recorder.durationMs(pcm) < 300) {
                showIdle()
                return
            }
            setWorking()
            worker.execute {
                val outcome = runCatching {
                    val raw = Dictation(prefs, secrets).recogniseWithGroq(pcm)
                    Dictation(prefs, secrets).finish(raw, prefs.language == "hinglish")
                }
                main.post { deliver(outcome) }
            }
        } else {
            recognizer?.stopListening()
            setWorking()
        }
    }

    private fun cancel() {
        if (prefs.engine == "groq") recorder.stop() else recognizer?.cancel()
        recognizer?.destroy()
        recognizer = null
        state = State.IDLE
    }

    private fun startPhoneRecognizer() {
        if (!SpeechRecognizer.isRecognitionAvailable(this)) {
            showMessage("This phone has no speech recogniser. Choose Groq in the app.", error = true)
            return
        }
        val r = SpeechRecognizer.createSpeechRecognizer(this)
        recognizer = r
        val tag = if (prefs.language == "hinglish") "hi-IN" else "en-IN"
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE, tag)
            putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, true)
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
        }
        r.setRecognitionListener(object : RecognitionListener {
            override fun onReadyForSpeech(params: Bundle?) = setListening()
            override fun onRmsChanged(rmsdB: Float) {
                levelBar.progress = ((rmsdB + 2f) * 8f).toInt().coerceIn(0, 100)
            }
            override fun onPartialResults(partial: Bundle?) {
                partial?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)?.firstOrNull()
                    ?.let { if (it.isNotBlank()) status.text = it }
            }
            override fun onResults(results: Bundle?) {
                val raw = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)?.firstOrNull().orEmpty()
                recognizer?.destroy()
                recognizer = null
                setWorking()
                worker.execute {
                    val outcome = runCatching { Dictation(prefs, secrets).finish(raw, prefs.language == "hinglish") }
                    main.post { deliver(outcome) }
                }
            }
            override fun onError(error: Int) {
                recognizer?.destroy()
                recognizer = null
                state = State.IDLE
                showMessage(
                    when (error) {
                        SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "Didn't catch that. Tap the mic and try again."
                        SpeechRecognizer.ERROR_NETWORK, SpeechRecognizer.ERROR_NETWORK_TIMEOUT -> "The phone's recogniser needs the internet for this language."
                        SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "Open the app and allow the microphone."
                        else -> "Speech recogniser error $error."
                    },
                    error = true,
                )
            }
            override fun onBeginningOfSpeech() = Unit
            override fun onBufferReceived(buffer: ByteArray?) = Unit
            override fun onEndOfSpeech() = Unit
            override fun onEvent(eventType: Int, params: Bundle?) = Unit
        })
        r.startListening(intent)
        setListening()
    }

    private fun deliver(outcome: Result<Dictation.Result>) {
        state = State.IDLE
        outcome.fold(
            onSuccess = { r ->
                if (r.text.isBlank()) {
                    showMessage("No speech heard.", error = false)
                } else {
                    type(r.text)
                    showMessage(r.notes.firstOrNull() ?: "Typed. Tap the mic to dictate again.", error = false)
                }
            },
            onFailure = { e -> showMessage(e.message ?: "Something went wrong.", error = true) },
        )
    }

    private fun pollLevel() {
        if (state != State.LISTENING) return
        levelBar.progress = (recorder.level * 400).toInt().coerceIn(0, 100)
        main.postDelayed({ pollLevel() }, 100)
    }

    // ---- typing ----

    private fun type(text: String) {
        val ic = currentInputConnection ?: return
        val before = ic.getTextBeforeCursor(1, 0)?.toString().orEmpty()
        val sep = if (before.isNotEmpty() && !before.last().isWhitespace()) " " else ""
        ic.commitText(sep + text, 1)
    }

    private fun delete() {
        val ic = currentInputConnection ?: return
        if (ic.getSelectedText(0).isNullOrEmpty()) ic.deleteSurroundingText(1, 0) else ic.commitText("", 1)
    }

    private fun deleteWord() {
        val ic = currentInputConnection ?: return
        val before = ic.getTextBeforeCursor(64, 0)?.toString().orEmpty()
        val trimmed = before.trimEnd()
        val cut = before.length - (trimmed.lastIndexOfAny(charArrayOf(' ', '\n')) + 1)
        ic.deleteSurroundingText(cut.coerceAtLeast(1), 0)
    }

    private fun enter() {
        val ic = currentInputConnection ?: return
        val action = currentInputEditorInfo?.imeOptions?.and(EditorInfo.IME_MASK_ACTION) ?: EditorInfo.IME_ACTION_NONE
        val noEnterAction = (currentInputEditorInfo?.imeOptions ?: 0) and EditorInfo.IME_FLAG_NO_ENTER_ACTION != 0
        if (action != EditorInfo.IME_ACTION_NONE && action != EditorInfo.IME_ACTION_UNSPECIFIED && !noEnterAction) {
            ic.performEditorAction(action)
        } else {
            ic.sendKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER))
            ic.sendKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_ENTER))
        }
    }

    private fun switchKeyboard() {
        if (state == State.LISTENING) cancel()
        val switched = Build.VERSION.SDK_INT >= Build.VERSION_CODES.P && switchToPreviousInputMethod()
        if (!switched) (getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager).showInputMethodPicker()
    }

    private fun cycleLanguage() {
        if (state != State.IDLE) return
        prefs.language = when (prefs.language) {
            "en-hinglish" -> "en"
            "en" -> "hinglish"
            else -> "en-hinglish"
        }
        showIdle()
    }

    // ---- display ----

    private fun languageLabel() = when (prefs.language) {
        "en" -> "English"
        "hinglish" -> "Hinglish"
        else -> "EN + Hinglish"
    }

    private fun showIdle() {
        state = State.IDLE
        mic.text = "🎤  Tap to speak"
        mic.background = rounded(getColor(R.color.accent))
        languageKey.text = languageLabel()
        levelBar.progress = 0
        val engine = if (prefs.engine == "groq") "Groq" else "phone recogniser"
        status.setTextColor(getColor(R.color.ink))
        status.text = "${languageLabel()} · $engine"
    }

    private fun setListening() {
        state = State.LISTENING
        mic.text = "■  Tap when done"
        mic.background = rounded(getColor(R.color.danger))
        status.setTextColor(getColor(R.color.ink))
        status.text = "Listening…"
    }

    private fun setWorking() {
        state = State.WORKING
        mic.text = "Working…"
        mic.background = rounded(getColor(R.color.accent_dark))
        levelBar.progress = 0
    }

    private fun showMessage(text: String, error: Boolean) {
        if (state != State.WORKING) {
            state = State.IDLE
        }
        showIdle()
        status.setTextColor(getColor(if (error) R.color.danger else R.color.ink))
        status.text = text
    }

    private fun rounded(color: Int) = GradientDrawable().apply {
        cornerRadius = dp(14).toFloat()
        setColor(color)
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()
}
