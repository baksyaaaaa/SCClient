package com.chromeclient;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * SC Client cosmetics: hats, capes, wings. Every item is a list of small coloured boxes, drawn flat in the menu
 * and as real 3D pieces on your player (see CosmeticLayer). Box units are half pixels: 2 units = 1 Minecraft pixel.
 * The head spans x 8..24, y 0..16, z -8..8 (front is negative z); the body spans x 8..24, y 16..40, z -4..4.
 */
public final class Cosmetics {
    public static final String[] CATS = {"Hats", "Capes", "Wings"};
    private static String[] capeNames() {
        String[] mine = {"None", "Midnight", "Snow", "Twisted", "Cherry"};
        String[] extra = {"Aurora", "Ember", "Abyss", "Galaxy", "Web"};   // newer ones go after the others so saved picks keep their number
        String[] all = new String[mine.length + CapeArt.NAMES.length + extra.length];
        System.arraycopy(mine, 0, all, 0, mine.length);
        System.arraycopy(CapeArt.NAMES, 0, all, mine.length, CapeArt.NAMES.length);
        System.arraycopy(extra, 0, all, mine.length + CapeArt.NAMES.length, extra.length);
        return all;
    }
    private static final String[] CAPES = capeNames();

    public static final String[][] ITEMS = {
            {"None", "Halo", "Cat ears", "Bunny ears", "Void ring"},
            CAPES,
            {"None", "Angel", "Void bat", "Butterfly", "Crystal", "Phoenix"},
            {"None", "Sparkles", "Hearts", "Flames", "Snow", "Magic", "Notes", "Soul fire", "Petals", "Electric", "Glow"},
    };
    public static final String[] WHERE = {"Around you", "Wings"};

    /** chosen item per category (0 = none); for Effects it is the particle type */
    public static final int[] chosen = new int[4];
    public static boolean effectOn = true;       // master switch for the particles
    public static int effectWhere = 0;
    public static float effectAmount = 0.5f;

    private Cosmetics() {}

    // ---------------- saving ----------------
    private static Path file() {
        return FabricLoader.getInstance().getConfigDir().resolve("chrome-cosmetics.json");
    }

    public static void load() {
        try {
            Path f = file();
            if (!Files.isRegularFile(f)) return;
            JsonObject o = JsonParser.parseString(Files.readString(f)).getAsJsonObject();
            for (int i = 0; i < CATS.length; i++) {
                String k = CATS[i].toLowerCase();
                if (o.has(k)) chosen[i] = Math.max(0, Math.min(ITEMS[i].length - 1, o.get(k).getAsInt()));
            }
            if (o.has("effectOn")) effectOn = o.get("effectOn").getAsBoolean();
            if (o.has("effectWhere")) effectWhere = Math.max(0, Math.min(WHERE.length - 1, o.get("effectWhere").getAsInt()));
            if (o.has("effectAmount")) effectAmount = Math.max(0.1f, Math.min(1f, o.get("effectAmount").getAsFloat()));
            CosmeticSync.fromJson(o);
        } catch (Exception ignored) {
            // defaults
        }
    }

    public static void save() {
        try {
            JsonObject o = new JsonObject();
            for (int i = 0; i < CATS.length; i++) o.addProperty(CATS[i].toLowerCase(), chosen[i]);
            o.addProperty("effectOn", effectOn);
            o.addProperty("effectWhere", effectWhere);
            o.addProperty("effectAmount", effectAmount);
            CosmeticSync.toJson(o);
            Files.writeString(file(), o.toString());
        } catch (Exception ignored) {
            // can't save: still works this session
        }
    }

    public static void init() {
        ClientTickEvents.END_CLIENT_TICK.register(Cosmetics::tick);
    }

    // ---------------- the pieces ----------------
    /** one box: x, y, w, h in half pixels (like the menu drawing), d = thickness, z = where it starts (front is negative) */
    public record Box(int x, int y, int w, int h, float d, float z, int color) {}

    private static final class L {
        final List<Box> l = new ArrayList<>();

        L a(int x, int y, int w, int h, float d, float z, int c) {
            l.add(new Box(x, y, w, h, d, z, c));
            return this;
        }

        /** the box and its mirror image on the other side of the middle (x = 16) */
        L m(int x, int y, int w, int h, float d, float z, int c) {
            a(x, y, w, h, d, z, c);
            return a(32 - x - w, y, w, h, d, z, c);
        }
    }

    private static int sh(int c, float f) {
        int r = Math.min(255, Math.round(((c >> 16) & 0xFF) * f)), g = Math.min(255, Math.round(((c >> 8) & 0xFF) * f)), b = Math.min(255, Math.round((c & 0xFF) * f));
        return 0xFF000000 | (r << 16) | (g << 8) | b;
    }

    private static int mix(int a, int b, float t) {
        int r = Math.round(((a >> 16) & 0xFF) * (1 - t) + ((b >> 16) & 0xFF) * t);
        int g = Math.round(((a >> 8) & 0xFF) * (1 - t) + ((b >> 8) & 0xFF) * t);
        int bl = Math.round((a & 0xFF) * (1 - t) + (b & 0xFF) * t);
        return 0xFF000000 | (r << 16) | (g << 8) | bl;
    }

    /** flat menu preview of hat i, taken from the real 3D planks (see HatData) */
    public static List<Box> hatBoxes(int i) {
        List<Box> out = new ArrayList<>();
        for (int[] b : HatData.preview(i)) out.add(new Box(b[0], b[1], b[2], b[3], 1f, -5f, b[4]));
        return out;
    }

    /** cape strips from top to bottom, each a little darker/lighter: a smooth gradient made of boxes */
    private static void gradientCape(L o, int top, int bottom, int rows) {
        int h = 44 / rows;
        for (int k = 0; k < rows; k++) {
            o.a(7, 16 + k * h, 18, h, 1.2f, 4.6f, mix(top, bottom, k / (float) (rows - 1)));
        }
    }

    public static List<Box> capeBoxes(int i) {
        L o = new L();
        switch (i) {
            case 1 -> {   // midnight with stars
                gradientCape(o, 0xFF1B1B4A, 0xFF0A0A22, 9);
                int[][] st = {{9, 22}, {20, 26}, {13, 33}, {22, 40}, {10, 46}, {17, 52}, {23, 55}, {12, 57}};
                for (int[] s : st) o.a(s[0], s[1], 2, 2, 0.5f, 5.9f, 0xFFFFF4B0);
                o.a(6, 59, 20, 2, 1.6f, 4.5f, 0xFF8B6CFF);
                o.m(6, 16, 2, 44, 1.6f, 4.5f, 0xFF8B6CFF);
            }
            case 2 -> {   // snow with fur collar
                gradientCape(o, 0xFFFFFFFF, 0xFFB5D3F0, 9);
                o.a(5, 14, 22, 6, 2.2f, 4.3f, 0xFFFFFFFF);
                for (int k = 0; k < 6; k++) o.a(6 + k * 3 + (k % 2), 19, 2, 2, 1.0f, 5.5f, 0xFFDCE8F6);
                o.a(6, 59, 20, 2, 1.6f, 4.5f, 0xFF8FB4E8);
            }
            case 3 -> {   // twisted: the real cape, pixel for pixel
                o.a(6, 16, 8, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(14, 16, 4, 3, 1.2f, 4.6f, 0xFFFDFBA6);
                o.a(18, 16, 8, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 19, 2, 3, 1.2f, 4.6f, 0xFF517E67);
                o.a(8, 19, 2, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(10, 19, 4, 3, 1.2f, 4.6f, 0xFFCFDDA0);
                o.a(14, 19, 4, 3, 1.2f, 4.6f, 0xFFFDFBA6);
                o.a(18, 19, 4, 3, 1.2f, 4.6f, 0xFFCFDDA0);
                o.a(22, 19, 2, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(24, 19, 2, 3, 1.2f, 4.6f, 0xFF517E67);
                o.a(6, 22, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(8, 22, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(14, 22, 4, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(18, 22, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(24, 22, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 25, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(8, 25, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(14, 25, 4, 3, 1.2f, 4.6f, 0xFFFDFBA6);
                o.a(18, 25, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(24, 25, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 28, 4, 3, 1.2f, 4.6f, 0xFF517E67);
                o.a(10, 28, 4, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(14, 28, 4, 3, 1.2f, 4.6f, 0xFFFDFBA6);
                o.a(18, 28, 4, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(22, 28, 4, 3, 1.2f, 4.6f, 0xFF517E67);
                o.a(6, 31, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(8, 31, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(14, 31, 4, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(18, 31, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(24, 31, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 34, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(8, 34, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(14, 34, 4, 3, 1.2f, 4.6f, 0xFFFDFBA6);
                o.a(18, 34, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(24, 34, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 37, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(8, 37, 2, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(10, 37, 2, 3, 1.2f, 4.6f, 0xFF517E67);
                o.a(12, 37, 2, 3, 1.2f, 4.6f, 0xFFCFDDA0);
                o.a(14, 37, 4, 3, 1.2f, 4.6f, 0xFFFDFBA6);
                o.a(18, 37, 2, 3, 1.2f, 4.6f, 0xFFCFDDA0);
                o.a(20, 37, 2, 3, 1.2f, 4.6f, 0xFF517E67);
                o.a(22, 37, 2, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(24, 37, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 40, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(8, 40, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(14, 40, 4, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(18, 40, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(24, 40, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 43, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(8, 43, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(14, 43, 4, 3, 1.2f, 4.6f, 0xFFFDFBA6);
                o.a(18, 43, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(24, 43, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 46, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(8, 46, 4, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(12, 46, 2, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(14, 46, 4, 3, 1.2f, 4.6f, 0xFFFDFBA6);
                o.a(18, 46, 2, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(20, 46, 4, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(24, 46, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 49, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(8, 49, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(14, 49, 4, 3, 1.2f, 4.6f, 0xFFA0A39C);
                o.a(18, 49, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(24, 49, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 52, 4, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(10, 52, 2, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(12, 52, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(14, 52, 4, 3, 1.2f, 4.6f, 0xFFCFDDA0);
                o.a(18, 52, 2, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(20, 52, 6, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 55, 8, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(14, 55, 6, 3, 1.2f, 4.6f, 0xFF0D1925);
                o.a(20, 55, 6, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(6, 58, 2, 3, 1.2f, 4.6f, 0xFF2A5E73);
                o.a(8, 58, 4, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(12, 58, 4, 3, 1.2f, 4.6f, 0xFF2A5E73);
                o.a(16, 58, 4, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(20, 58, 2, 3, 1.2f, 4.6f, 0xFF2A5E73);
                o.a(22, 58, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(24, 58, 2, 3, 1.2f, 4.6f, 0xFF2A5E73);
                o.a(6, 61, 2, 3, 1.2f, 4.6f, 0xFF2A5E73);
                o.a(8, 61, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(10, 61, 6, 3, 1.2f, 4.6f, 0xFF2A5E73);
                o.a(16, 61, 2, 3, 1.2f, 4.6f, 0xFF152E44);
                o.a(18, 61, 8, 3, 1.2f, 4.6f, 0xFF2A5E73);
            }
            case 4 -> {   // cherry
                gradientCape(o, 0xFFFFD9E6, 0xFFFF9FC2, 9);
                int[][] bl = {{9, 24}, {20, 30}, {12, 38}, {21, 45}, {10, 52}};
                for (int[] p : bl) o.a(p[0], p[1], 3, 3, 0.5f, 5.9f, 0xFFFFFFFF);
                o.a(6, 59, 20, 2, 1.6f, 4.5f, 0xFFD26E96);
            }
            case 24 -> gradientCape(o, 0xFF0A2433, 0xFF2EE8A0, 9);
            case 25 -> gradientCape(o, 0xFF4A0B05, 0xFFF08A1C, 9);
            case 26 -> gradientCape(o, 0xFF0E5A7E, 0xFF020812, 9);
            case 27 -> gradientCape(o, 0xFF0B0620, 0xFF9A6CE0, 9);
            default -> { return CapeArt.boxes(i); }
        }
        return o.l;
    }

    /** one (left) wing, as seen from behind; the right one is its mirror image. y runs down from the shoulder. */
    /** flat menu preview of both wings of wing `i`, taken from the real 3D planks (see WingPlanks) */
    public static List<Box> wingBoxes(int i) {
        List<Box> out = new ArrayList<>();
        for (int[] b : WingPlanks.preview(i)) out.add(new Box(b[0], b[1], b[2], b[3], 1f, 8f, b[4]));
        return out;
    }

    public static List<Box> mirrored(List<Box> left) {
        List<Box> out = new ArrayList<>();
        for (Box b : left) out.add(new Box(32 - b.x() - b.w(), b.y(), b.w(), b.h(), b.d(), b.z(), b.color()));
        return out;
    }

    // ---------------- picks changed in the launcher while the game runs ----------------
    private static long seenModified = -1;
    private static int ticks = 0;

    /** every 2 seconds: if the launcher's Wardrobe wrote new picks, wear them right away */
    private static void tick(Minecraft mc) {
        if (++ticks % 40 != 0) return;
        try {
            Path f = file();
            long m = Files.isRegularFile(f) ? Files.getLastModifiedTime(f).toMillis() : -1;
            if (seenModified == -1) { seenModified = m; return; }
            if (m != seenModified) { seenModified = m; load(); }
        } catch (Exception ignored) { }
    }
}
