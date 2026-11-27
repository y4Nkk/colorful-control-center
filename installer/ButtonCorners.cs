using System;
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace ColorfulControlCenter.Installer
{
    public sealed class ButtonCorners : IMultiValueConverter
    {
        public static readonly DependencyProperty RadiusProperty = DependencyProperty.RegisterAttached(
            "Radius", typeof(CornerRadius), typeof(ButtonCorners), new PropertyMetadata(new CornerRadius(0)));

        public static CornerRadius GetRadius(DependencyObject element)
        {
            return (CornerRadius)element.GetValue(RadiusProperty);
        }

        public static void SetRadius(DependencyObject element, CornerRadius radius)
        {
            element.SetValue(RadiusProperty, radius);
        }

        public object Convert(object[] values, Type targetType, object parameter, CultureInfo culture)
        {
            if (!(values[0] is CornerRadius) || !(values[1] is double) || !(values[2] is double))
                return DependencyProperty.UnsetValue;

            // CSS scales the uniform radii together when the corners meet.
            double limit = Math.Min((double)values[1], (double)values[2]) / 2;
            return new CornerRadius(Math.Min(((CornerRadius)values[0]).TopLeft, limit));
        }

        public object[] ConvertBack(object value, Type[] targetTypes, object parameter, CultureInfo culture)
        {
            throw new NotSupportedException();
        }
    }
}
