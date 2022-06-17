using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Data;
using ConvertZZ.Core.Helpers;

namespace ConvertZZ.Converters
{
    public class RadioBoolETextConvertModeConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is ETextConvertMode eTextConvertMode)
            {
                return eTextConvertMode.ToString() == parameter.ToString();
            }
            throw new NotImplementedException();
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            return Enum.Parse<ETextConvertMode>((string)parameter);
        }
    }
}