package cc.straysmp.capes.menu;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;

/**
 * The Stray look for Minecraft's own buttons (pause menu, options, title screen …): dark
 * panels with a soft edge and the Stray blue on hover, instead of the grey stone buttons.
 * Follows the "Stray design" switch (on by default).
 */
public final class StrayStyle {
    private static final int BG = 0xE61E1F28;
    private static final int BG_HOVER = 0xF0262A3A;
    private static final int BG_OFF = 0x991A1B24;
    private static final int EDGE = 0x33FFFFFF;
    private static final int ACCENT = 0xFF51A2FF;
    private static final int TEXT = 0xFFFFFFFF;
    private static final int TEXT_OFF = 0xFF7D8195;

    private StrayStyle() {}

    public static boolean on() {
        return ClientConfig.strayDesign();
    }

    /** Draws a button in the Stray look (background, edge, centred label). */
    public static void button(GuiGraphicsExtractor g, AbstractWidget w) {
        int x = w.getX(), y = w.getY(), wd = w.getWidth(), h = w.getHeight();
        boolean hot = w.active && w.isHoveredOrFocused();
        int bg = !w.active ? BG_OFF : hot ? BG_HOVER : BG;
        // body with cut corners (reads as rounded at GUI scale)
        g.fill(x + 1, y, x + wd - 1, y + h, bg);
        g.fill(x, y + 1, x + 1, y + h - 1, bg);
        g.fill(x + wd - 1, y + 1, x + wd, y + h - 1, bg);
        // edge: soft light, blue when hovered
        int edge = hot ? ACCENT : EDGE;
        g.fill(x + 1, y, x + wd - 1, y + 1, edge);
        g.fill(x + 1, y + h - 1, x + wd - 1, y + h, hot ? ACCENT : 0x22000000);
        g.fill(x, y + 1, x + 1, y + h - 1, edge);
        g.fill(x + wd - 1, y + 1, x + wd, y + h - 1, edge);
        if (hot) g.fill(x + 2, y + h - 3, x + wd - 2, y + h - 1, 0x5551A2FF); // glow under the label
        Font font = Minecraft.getInstance().font;
        int color = w.active ? TEXT : TEXT_OFF;
        int tw = font.width(w.getMessage());
        int maxW = wd - 8;
        if (tw <= maxW) {
            g.centeredText(font, w.getMessage(), x + wd / 2, y + (h - 8) / 2, color);
        } else {
            g.enableScissor(x + 4, y, x + wd - 4, y + h);
            g.text(font, w.getMessage(), x + 4, y + (h - 8) / 2, color);
            g.disableScissor();
        }
    }
}
