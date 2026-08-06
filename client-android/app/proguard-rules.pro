# kotlinx.serialization — los serializadores se generan en compilación y se
# alcanzan por reflexión desde el companion; sin esto R8 los borra.
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**
-keepclassmembers class **$$serializer { *; }
-keepclasseswithmembers class ** {
    public static ** Companion;
    kotlinx.serialization.KSerializer serializer(...);
}
-keep,includedescriptorclasses class cl.rutbusiness.**$$serializer { *; }
-keepclassmembers class cl.rutbusiness.** {
    *** Companion;
}

# Ktor / OkHttp: referencias opcionales a clases que no están en Android.
-dontwarn org.slf4j.**
-dontwarn org.conscrypt.**
-dontwarn org.bouncycastle.**
-dontwarn org.openjsse.**
-dontwarn java.lang.management.**
