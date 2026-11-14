package app.voice

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Settings on this phone. Everything is off or local until the user turns it on. */
class Prefs(context: Context) {
    private val p = context.applicationContext.getSharedPreferences("settings", Context.MODE_PRIVATE)

    /** "en-hinglish" (decided per dictation), "en" or "hinglish". */
    var language: String
        get() = p.getString("language", "en-hinglish") ?: "en-hinglish"
        set(v) = p.edit().putString("language", v).apply()

    /** "phone" (the phone's own recogniser) or "groq" (the user's own Groq key). */
    var engine: String
        get() = p.getString("engine", "phone") ?: "phone"
        set(v) = p.edit().putString("engine", v).apply()

    /** The user agreed that dictation audio is sent to Groq for recognition. */
    var audioConsent: Boolean
        get() = p.getBoolean("audio_consent", false)
        set(v) = p.edit().putBoolean("audio_consent", v).apply()

    /** The user agreed that dictation text is sent to Groq for clean-up and Hinglish spelling. */
    var textConsent: Boolean
        get() = p.getBoolean("text_consent", false)
        set(v) = p.edit().putBoolean("text_consent", v).apply()

    /** LIGHT, PROFESSIONAL or LITERAL, as on the desktop. */
    var level: String
        get() = p.getString("level", "LIGHT") ?: "LIGHT"
        set(v) = p.edit().putString("level", v).apply()

    val vocabulary: String
        get() = "{\"terms\": ${Core.defaultTerms()}}"
}

/**
 * The provider key, encrypted with an AES key that never leaves the Android Keystore.
 * Only the encrypted bytes are stored in the app's private settings.
 */
class SecretStore(context: Context) {
    private val p = context.applicationContext.getSharedPreferences("secrets", Context.MODE_PRIVATE)

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build(),
        )
        return gen.generateKey()
    }

    fun save(secret: String) {
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.ENCRYPT_MODE, key())
        val ct = c.doFinal(secret.trim().toByteArray(Charsets.UTF_8))
        p.edit()
            .putString("iv", Base64.encodeToString(c.iv, Base64.NO_WRAP))
            .putString("ct", Base64.encodeToString(ct, Base64.NO_WRAP))
            .apply()
    }

    fun load(): String? {
        val iv = p.getString("iv", null) ?: return null
        val ct = p.getString("ct", null) ?: return null
        return try {
            val c = Cipher.getInstance("AES/GCM/NoPadding")
            c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, Base64.decode(iv, Base64.NO_WRAP)))
            String(c.doFinal(Base64.decode(ct, Base64.NO_WRAP)), Charsets.UTF_8)
        } catch (e: Exception) {
            null
        }
    }

    fun clear() = p.edit().clear().apply()

    private companion object {
        const val ALIAS = "provider-key"
    }
}
