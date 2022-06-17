using Fanhuaji_API;
using Fanhuaji_API.Enum;
using PropertyChanged;

namespace ConvertZZ.Class
{
    [AddINotifyPropertyChangedInterface]
    public class Fanhuaji_Config : Config
    {
        [ColumnName(Visible: false)]
        public int Converter_T_to_S_Index
        {
            get { return (int)Converter_T_to_S; }
            set { Converter_T_to_S = (Enum_Converter)value; }
        }

        [ColumnName(Visible: false)]
        public int Converter_S_to_T_Index
        {
            get { return (int)Converter_S_to_T; }
            set { Converter_S_to_T = (Enum_Converter)value; }
        }

        [ColumnName(Visible: false)]
        public Enum_Converter Converter_T_to_S { get; set; } = Enum_Converter.Simplified;

        [ColumnName(Visible: false)]
        public Enum_Converter Converter_S_to_T { get; set; } = Enum_Converter.Taiwan;

        [ColumnName(Visible: false)]
        public int TranslateTabsToSpaces_Index
        {
            get
            {
                return TranslateTabsToSpaces + 1;
            }
            set
            {
                TranslateTabsToSpaces = value - 1;
            }
        }

        [ColumnName(Visible: false)]
        public int JpStyleConversionStrategy_Index
        {
            get
            {
                return (int)JpStyleConversionStrategy;
            }
            set
            {
                JpStyleConversionStrategy = (Enum_jpConversionStrategy)value;
            }
        }

        [ColumnName(Visible: false)]
        public int JpTextConversionStrategy_Index
        {
            get
            {
                return (int)JpTextConversionStrategy;
            }
            set
            {
                JpTextConversionStrategy = (Enum_jpConversionStrategy)value;
            }
        }
    }
}