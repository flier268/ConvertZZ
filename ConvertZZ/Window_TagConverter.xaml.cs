using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;

namespace ConvertZZ
{
    /// <summary>
    /// Window_TagConverter.xaml 的互動邏輯
    /// </summary>
    public partial class Window_TagConverter : Window
    {
        public Window_TagConverter(Format Format)
        {
            InitializeComponent();
        }
        public enum Format
        {
            ID3,
            APE,
            OGG
        }
    }
}
