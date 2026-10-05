import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import javax.inject.Inject

plugins {
  alias(libs.plugins.android.application)
  alias(libs.plugins.kotlin.compose)
}

// The Rust core (crates/connect) lives in the EchoFiles workspace one level up.
val repoRoot: File = rootDir.parentFile
val ndkVersion = "30.0.16248370"

/** Builds libechoconnect.so for the phone (arm64) and the emulator (x86_64). */
abstract class CargoNdk @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
  @get:OutputDirectory abstract val outputDir: DirectoryProperty
  @get:org.gradle.api.tasks.Internal lateinit var workspace: File
  @get:org.gradle.api.tasks.Internal lateinit var ndkHome: String
  @get:org.gradle.api.tasks.Input var release: Boolean = true

  @TaskAction
  fun build() {
    exec.exec {
      workingDir = workspace
      environment("ANDROID_NDK_HOME", ndkHome)
      val args = mutableListOf("ndk", "-t", "arm64-v8a", "-t", "x86_64", "-P", "29", "-o", outputDir.get().asFile.absolutePath, "build", "-p", "echoconnect-core", "--lib")
      if (release) args += "--release"
      commandLine(listOf("cargo") + args)
    }
  }
}

/** Generates the Kotlin side of the UniFFI bridge from the host build of the same crate. */
abstract class UniffiKotlin @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
  @get:OutputDirectory abstract val outputDir: DirectoryProperty
  @get:org.gradle.api.tasks.Internal lateinit var workspace: File

  @TaskAction
  fun generate() {
    exec.exec {
      workingDir = workspace
      commandLine("cargo", "build", "-p", "echoconnect-core", "--lib")
    }
    exec.exec {
      workingDir = workspace
      commandLine(
        "cargo", "run", "-q", "-p", "echoconnect-core", "--bin", "uniffi-bindgen", "--",
        "generate", "--library", "target/debug/libechoconnect.so", "--language", "kotlin",
        "--no-format", "--out-dir", outputDir.get().asFile.absolutePath,
      )
    }
  }
}

val cargoNdk = tasks.register<CargoNdk>("cargoNdk") {
  workspace = repoRoot
  ndkHome = "${System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT") ?: "${System.getProperty("user.home")}/Android/Sdk"}/ndk/$ndkVersion"
  outputDir.set(layout.buildDirectory.dir("rust/jniLibs"))
  outputs.upToDateWhen { false }
}

val uniffi = tasks.register<UniffiKotlin>("uniffiKotlin") {
  workspace = repoRoot
  outputDir.set(layout.buildDirectory.dir("generated/uniffi"))
  outputs.upToDateWhen { false }
}

android {
  namespace = "app.echoconnect"
  compileSdk = 37
  ndkVersion = "30.0.16248370"

  defaultConfig {
    applicationId = "app.echoconnect"
    minSdk = 29
    targetSdk = 36
    versionCode = System.getenv("VERSION_CODE")?.toIntOrNull() ?: 1
    versionName = System.getenv("VERSION_NAME") ?: "0.1.0"
    ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
  }

  signingConfigs {
    create("release") {
      val path = System.getenv("ECHOCONNECT_KEYSTORE")
      if (path != null) {
        storeFile = file(path)
        storePassword = System.getenv("ECHOCONNECT_KEYSTORE_PASSWORD")
        keyAlias = System.getenv("ECHOCONNECT_KEY_ALIAS") ?: "echoconnect"
        keyPassword = System.getenv("ECHOCONNECT_KEY_PASSWORD")
      }
    }
  }

  buildTypes {
    release {
      isMinifyEnabled = false
      if (System.getenv("ECHOCONNECT_KEYSTORE") != null) signingConfig = signingConfigs.getByName("release")
    }
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
  buildFeatures {
    compose = true
    buildConfig = true
  }
  packaging { jniLibs { useLegacyPackaging = false } }
}

androidComponents {
  onVariants { variant ->
    variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoNdk, CargoNdk::outputDir)
    variant.sources.kotlin?.addGeneratedSourceDirectory(uniffi, UniffiKotlin::outputDir)
  }
}

dependencies {
  implementation(libs.androidx.core.ktx)
  implementation(libs.androidx.activity.compose)
  implementation(platform(libs.androidx.compose.bom))
  implementation(libs.androidx.compose.ui)
  implementation(libs.androidx.compose.foundation)
  implementation(libs.androidx.compose.material3)
  implementation(libs.androidx.compose.ui.tooling.preview)
  implementation(libs.androidx.lifecycle.runtime.compose)
  implementation(libs.androidx.lifecycle.service)
  implementation(libs.androidx.camera.core)
  implementation(libs.androidx.camera.camera2)
  implementation(libs.androidx.camera.lifecycle)
  implementation(libs.androidx.camera.view)
  implementation(libs.mlkit.barcode)
  implementation("com.google.android.gms:play-services-mlkit-document-scanner:16.0.0")
  implementation(libs.androidx.work)
  implementation("${libs.jna.get().module}:${libs.jna.get().version}@aar")
}
