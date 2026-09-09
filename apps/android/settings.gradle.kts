pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
// Missing review files must fail instead of silently resolving an unlocked build.
listOf("buildscript-gradle.lockfile", "app/buildscript-gradle.lockfile",
    "app/gradle.lockfile", "gradle/verification-metadata.xml").forEach {
    check(file(it).isFile) { "Required reviewed Android dependency file is missing: $it" }
}
check(gradle.startParameter.dependencyVerificationMode == org.gradle.api.artifacts.verification.DependencyVerificationMode.STRICT) {
    "Android builds require strict dependency checksum verification"
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories { google(); mavenCentral() }
}
rootProject.name = "Tundra"
include(":app")
