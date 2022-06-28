using System.Text.Json.Serialization;
using Fanhuaji_API.Enum;
using Fanhuaji_API.Models;
using PropertyChanged;

namespace ConvertZZ.Class
{
    [AddINotifyPropertyChangedInterface]
    public class Fanhuaji_Config : Config
    {
        [JsonIgnore]
        public int Converter_T_to_S_Index
        {
            get { return (int)Converter_T_to_S; }
            set { Converter_T_to_S = (Enum_Converter)value; }
        }

        [JsonIgnore]
        public int Converter_S_to_T_Index
        {
            get { return (int)Converter_S_to_T; }
            set { Converter_S_to_T = (Enum_Converter)value; }
        }

        public Enum_Converter Converter_T_to_S { get; set; } = Enum_Converter.Simplified;

        public Enum_Converter Converter_S_to_T { get; set; } = Enum_Converter.Traditional;

        [JsonIgnore]
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

        [JsonIgnore]
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

        [JsonIgnore]
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