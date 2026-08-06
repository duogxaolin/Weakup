import java.util.Properties

plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
    // Consumes `google-services.json` in this directory. The file identifies the
    // Firebase project rather than authenticating anyone — Firestore rules are
    // what protect the data — so it is committed deliberately.
    id("com.google.gms.google-services")
}

// Release signing material, when it exists. `key.properties` is gitignored and is
// absent from a fresh clone; the release workflow writes it from repository secrets
// when those secrets are configured.
//
// Read here rather than inside `buildTypes` so the fallback can be announced during
// configuration, where the message lands in the build log whether or not the release
// variant is the one being assembled.
val keystorePropertiesFile = rootProject.file("key.properties")
val keystoreProperties =
    Properties().apply {
        if (keystorePropertiesFile.exists()) {
            keystorePropertiesFile.inputStream().use { load(it) }
        }
    }
val hasReleaseKeystore =
    keystorePropertiesFile.exists() &&
        keystoreProperties.getProperty("storeFile") != null &&
        rootProject.file(keystoreProperties.getProperty("storeFile")).exists()

// Read into locals here, at the top level, rather than inside
// `signingConfigs.create("release") { ... }`: within that block the receiver is a
// SigningConfig and the enclosing script's declarations are no longer in scope, so
// `keystoreProperties.getProperty` does not resolve there.
val releaseStoreFile = keystoreProperties.getProperty("storeFile")
val releaseStorePassword = keystoreProperties.getProperty("storePassword")
val releaseKeyAlias = keystoreProperties.getProperty("keyAlias")
val releaseKeyPassword = keystoreProperties.getProperty("keyPassword")

android {
    namespace = "vn.delify.weakup"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
        isCoreLibraryDesugaringEnabled = true
    }

    defaultConfig {
        // TODO: Specify your own unique Application ID (https://developer.android.com/studio/build/application-id.html).
        applicationId = "vn.delify.weakup"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    signingConfigs {
        // Declared only when the material is actually present. Declaring it
        // unconditionally with null paths makes Gradle fail at configuration time
        // with a message about a missing file rather than about a missing secret.
        if (hasReleaseKeystore) {
            create("release") {
                storeFile = rootProject.file(releaseStoreFile)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        release {
            // Signed with the real upload key when one is configured, and with the
            // Android debug key otherwise.
            //
            // The debug fallback is what makes `flutter build apk --release` work on a
            // fresh clone, but a debug-signed APK is sideload-only: its signature is
            // generated from a well-known key that every SDK installation shares, so it
            // identifies nobody, and Play Store upload rejects it. The warning below is
            // deliberately not silent — a release published without noticing the
            // fallback is the failure mode worth being noisy about.
            signingConfig =
                if (hasReleaseKeystore) {
                    signingConfigs.getByName("release")
                } else {
                    logger.warn(
                        "WARNING: no key.properties found — signing the release build with the " +
                            "DEBUG key. The resulting APK can be sideloaded but is NOT " +
                            "Play-Store publishable and its signature does not identify the " +
                            "developer.",
                    )
                    signingConfigs.getByName("debug")
                }
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}

dependencies {
    coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:2.1.4")
}
