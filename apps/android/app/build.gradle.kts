plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}
android {
    namespace = "dev.johnny9.tundra"
    compileSdk = 36
    buildToolsVersion = "35.0.0"
    ndkVersion = "27.2.12479018"
    defaultConfig {
        applicationId = "dev.johnny9.tundra.dev"
        minSdk = 28
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0-dev.1"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    packaging { jniLibs.useLegacyPackaging = false }
    buildTypes { release { isMinifyEnabled = false } }
    sourceSets.getByName("main").assets.srcDir("../../../third-party/bundle")
    sourceSets.getByName("test").resources.srcDir("../../../tests/fixtures")
    sourceSets.getByName("androidTest").assets.srcDir("../../../tests/fixtures")
}
tasks.withType<Test>().configureEach {
    // Exercise the host Rust library via JNA, not an Android ABI or a mocked wallet.
    systemProperty("jna.library.path", providers.gradleProperty("tundra.hostLibraryDir")
        .getOrElse(rootProject.projectDir.resolve("../../target/debug").absolutePath))
}
dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.12.00"))
    // Kotlin's debugImplementationDependenciesMetadata resolves this scope on its
    // own. Give its versionless tooling the same BOM used by the app classpaths.
    debugImplementation(platform("androidx.compose:compose-bom:2025.12.00"))
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.0")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
    implementation("androidx.camera:camera-camera2:1.6.2")
    implementation("androidx.camera:camera-lifecycle:1.6.2")
    implementation("androidx.camera:camera-view:1.6.2")
    implementation("com.google.zxing:core:3.5.4")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation(platform("androidx.compose:compose-bom:2025.12.00"))
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test:core-ktx:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    // The Android AAR has no host JNA dispatch library; JVM tests need the desktop artifact.
    testRuntimeOnly("net.java.dev.jna:jna:5.17.0@jar")
    debugImplementation("androidx.compose.ui:ui-tooling")
    // Supplies ComponentActivity for standalone Compose instrumentation tests only.
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
