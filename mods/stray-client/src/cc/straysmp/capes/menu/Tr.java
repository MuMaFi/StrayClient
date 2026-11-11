package cc.straysmp.capes.menu;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;

import cc.straysmp.capes.StrayCapes;
import com.google.gson.JsonElement;
import com.google.gson.JsonParser;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;

/**
 * Our own texts, read straight from the mod jar ({@code assets/strayclient/lang}). Minecraft
 * only loads a mod's language files when Fabric API is installed, and a profile without the
 * standard mods doesn't have it, so the menus don't rely on that.
 */
public final class Tr {
    private static String loadedFor;
    private static Map<String, String> texts = Map.of();
    private static Map<String, String> english;

    private Tr() {}

    public static MutableComponent t(String key) {
        return Component.literal(s(key));
    }

    public static synchronized String s(String key) {
        String lang = "en_us";
        try {
            lang = Minecraft.getInstance().options.languageCode;
        } catch (RuntimeException ignored) {
        }
        if (!lang.equals(loadedFor)) {
            loadedFor = lang;
            texts = read(lang);
        }
        String v = texts.get(key);
        if (v != null) return v;
        if (english == null) english = read("en_us");
        return english.getOrDefault(key, key);
    }

    private static Map<String, String> read(String lang) {
        Map<String, String> out = new HashMap<>();
        if (!lang.matches("[a-z0-9_]{2,16}")) return out;
        try (InputStream in = StrayCapes.class.getResourceAsStream("/assets/strayclient/lang/" + lang + ".json")) {
            if (in == null) return out;
            try (Reader r = new InputStreamReader(in, StandardCharsets.UTF_8)) {
                for (Map.Entry<String, JsonElement> e : JsonParser.parseReader(r).getAsJsonObject().entrySet()) {
                    out.put(e.getKey(), e.getValue().getAsString());
                }
            }
        } catch (IOException | RuntimeException e) {
            StrayCapes.LOG.warn("Could not read language {}", lang, e);
        }
        return out;
    }
}
