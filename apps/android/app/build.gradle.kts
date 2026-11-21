import groovy.json.JsonSlurper

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// The product name and id live only in /brand.json.
@Suppress("UNCHECKED_CAST")
val brand = JsonSlurper().parse(rootProject.file("../../brand.json")) as Map<String, String>

android {
    namespace = "app.voice"
    compileSdk = 35

    defaultConfig {
        applicationId = brand.getValue("appId")
        minSdk = 26
        targetSdk = 35
        versionCode = 9
        versionName = "0.4.5"
        resValue("string", "app_name", brand.getValue("appName"))
        ndk {
            // Built by cargo-ndk into src/main/jniLibs (see .github/workflows/android.yml).
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }

    testOptions {
        unitTests.all {
            // The JVM tests load the host build of the shared Rust library.
            it.systemProperty("java.library.path", System.getenv("HOST_LIB_DIR") ?: "")
            it.systemProperty("fixtures.dir", rootProject.file("../../spec/fixtures").absolutePath)
        }
    }
}

dependencies {
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}
