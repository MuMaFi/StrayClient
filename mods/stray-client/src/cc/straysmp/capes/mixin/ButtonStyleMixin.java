package cc.straysmp.capes.mixin;

import cc.straysmp.capes.menu.StrayStyle;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Minecraft's plain buttons in the Stray look (see StrayStyle). */
@Mixin(targets = "net.minecraft.client.gui.components.Button$Plain")
public abstract class ButtonStyleMixin {
    @Inject(method = "extractContents", at = @At("HEAD"), cancellable = true, require = 0)
    private void strayclient$style(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta, CallbackInfo ci) {
        if (!StrayStyle.on()) return;
        StrayStyle.button(g, (AbstractWidget) (Object) this);
        ci.cancel();
    }
}
