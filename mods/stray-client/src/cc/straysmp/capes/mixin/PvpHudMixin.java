package cc.straysmp.capes.mixin;

import cc.straysmp.capes.menu.HudOffsets;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Lets the player move the PvP Plus HUD (it draws at fixed places): the info block with the
 * effects list, and the armor block with the item counters, are shifted by the offsets set in
 * the Stray Client menu. Optional: skipped when PvP Plus isn't installed or changes its HUD.
 */
@Pseudo
@Mixin(targets = "de.pvpplus.PvpHud", remap = false)
public abstract class PvpHudMixin {
    private static void strayclient$push(GuiGraphicsExtractor g, int dx, int dy) {
        HudOffsets.load();
        g.pose().pushMatrix();
        g.pose().translate(dx, dy);
    }

    @Inject(method = "drawInfo", at = @At("HEAD"), require = 0)
    private static void strayclient$infoIn(GuiGraphicsExtractor g, Minecraft mc, Font font, Player player, int y, CallbackInfoReturnable<Integer> cir) {
        strayclient$push(g, HudOffsets.infoDx, HudOffsets.infoDy);
    }

    @Inject(method = "drawInfo", at = @At("RETURN"), require = 0)
    private static void strayclient$infoOut(GuiGraphicsExtractor g, Minecraft mc, Font font, Player player, int y, CallbackInfoReturnable<Integer> cir) {
        g.pose().popMatrix();
    }

    @Inject(method = "drawEffects", at = @At("HEAD"), require = 0)
    private static void strayclient$effectsIn(GuiGraphicsExtractor g, Font font, Player player, int y, CallbackInfo ci) {
        strayclient$push(g, HudOffsets.infoDx, HudOffsets.infoDy);
    }

    @Inject(method = "drawEffects", at = @At("RETURN"), require = 0)
    private static void strayclient$effectsOut(GuiGraphicsExtractor g, Font font, Player player, int y, CallbackInfo ci) {
        g.pose().popMatrix();
    }

    @Inject(method = "drawArmor", at = @At("HEAD"), require = 0)
    private static void strayclient$armorIn(GuiGraphicsExtractor g, Font font, Player player, int x, int y, CallbackInfoReturnable<Integer> cir) {
        strayclient$push(g, HudOffsets.armorDx, HudOffsets.armorDy);
    }

    @Inject(method = "drawArmor", at = @At("RETURN"), require = 0)
    private static void strayclient$armorOut(GuiGraphicsExtractor g, Font font, Player player, int x, int y, CallbackInfoReturnable<Integer> cir) {
        g.pose().popMatrix();
    }

    @Inject(method = "drawItems", at = @At("HEAD"), require = 0)
    private static void strayclient$itemsIn(GuiGraphicsExtractor g, Font font, Player player, int x, int y, float delta, CallbackInfo ci) {
        strayclient$push(g, HudOffsets.armorDx, HudOffsets.armorDy);
    }

    @Inject(method = "drawItems", at = @At("RETURN"), require = 0)
    private static void strayclient$itemsOut(GuiGraphicsExtractor g, Font font, Player player, int x, int y, float delta, CallbackInfo ci) {
        g.pose().popMatrix();
    }
}
