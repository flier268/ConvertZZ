using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Controls;
using System.Windows.Data;

namespace ConvertZZ.Converters
{
    public class ID3v2OutputConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is ComboBoxItem comboBoxItem)
            {
                switch (comboBoxItem.Content)
                {
                    case "2.3":
                        return new List<Encoding>(new Encoding[] { Encoding.GetEncoding("UTF-16"), Encoding.GetEncoding("UTF-16LE"), Encoding.GetEncoding("UTF-16BE") });

                    case "2.4":
                        return new List<Encoding>(new Encoding[] { Encoding.GetEncoding("UTF-8"), Encoding.GetEncoding("UTF-16"), Encoding.GetEncoding("UTF-16LE"), Encoding.GetEncoding("UTF-16BE") });
                }
            }
            throw new NotImplementedException();
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            throw new NotImplementedException();
        }
    }
}