using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;

namespace ColorfulControlCenter.Installer
{
    internal sealed partial class InstallerWindow
    {
        private readonly TranslateTransform bodyOffset = new TranslateTransform();
        private FrameworkElement actions;
        private Border workspacePanel;
        private int transitionSerial;

        private bool MotionAllowed
        {
            get { return SystemParameters.ClientAreaAnimation && !SystemParameters.HighContrast
                && Measure("MotionMilliseconds") > 0; }
        }

        private void AnimateStep(int previousStep)
        {
            if (previousStep < 0 || !IsLoaded || !MotionAllowed)
            {
                transitionSerial++;
                body.BeginAnimation(OpacityProperty, null);
                bodyOffset.BeginAnimation(TranslateTransform.XProperty, null);
                actions.BeginAnimation(OpacityProperty, null);
                return;
            }
            if (previousStep == step) return;
            int serial = ++transitionSerial;
            double distance = Measure("MotionDistance") * (step > previousStep ? 1 : -1);
            var fade = Tween(0, 1, "MotionMilliseconds");
            var offset = Tween(distance, 0, "MotionMilliseconds");
            var actionFade = Tween(0.65, 1, "FastMotionMilliseconds");
            fade.Completed += delegate { if (serial == transitionSerial) body.BeginAnimation(OpacityProperty, null); };
            offset.Completed += delegate { if (serial == transitionSerial) bodyOffset.BeginAnimation(TranslateTransform.XProperty, null); };
            actionFade.Completed += delegate { if (serial == transitionSerial) actions.BeginAnimation(OpacityProperty, null); };
            body.BeginAnimation(OpacityProperty, fade, HandoffBehavior.SnapshotAndReplace);
            bodyOffset.BeginAnimation(TranslateTransform.XProperty, offset, HandoffBehavior.SnapshotAndReplace);
            actions.BeginAnimation(OpacityProperty, actionFade, HandoffBehavior.SnapshotAndReplace);
        }

        private DoubleAnimation Tween(double from, double to, string duration)
        {
            return new DoubleAnimation(from, to, TimeSpan.FromMilliseconds(Measure(duration))) {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }, FillBehavior = FillBehavior.Stop,
            };
        }

        private void UpdateMarker(Label marker, string color, double opacity)
        {
            var previous = marker.Background as SolidColorBrush;
            var target = new SolidColorBrush(((SolidColorBrush)Brush(color)).Color);
            double previousOpacity = marker.Opacity;
            marker.Background = target;
            marker.Opacity = opacity;
            if (previous == null || !IsLoaded || !MotionAllowed)
            {
                marker.BeginAnimation(OpacityProperty, null);
                return;
            }
            var fade = Tween(previousOpacity, opacity, "FastMotionMilliseconds");
            var tint = new ColorAnimation(previous.Color, target.Color,
                TimeSpan.FromMilliseconds(Measure("FastMotionMilliseconds"))) { FillBehavior = FillBehavior.Stop };
            fade.Completed += delegate {
                if (ReferenceEquals(marker.Background, target)) marker.BeginAnimation(OpacityProperty, null);
            };
            tint.Completed += delegate { target.BeginAnimation(SolidColorBrush.ColorProperty, null); };
            marker.BeginAnimation(OpacityProperty, fade, HandoffBehavior.SnapshotAndReplace);
            target.BeginAnimation(SolidColorBrush.ColorProperty, tint, HandoffBehavior.SnapshotAndReplace);
        }
    }
}
